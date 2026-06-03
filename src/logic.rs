// Pure state-transition logic for the pane application.
// No reactive or WASM dependencies — compiled for native (tests) and WASM alike.

/// Identifies which of the four panes currently holds keyboard focus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActivePane {
    Left,
    Middle,
    Right,
    Bottom,
}

impl std::fmt::Display for ActivePane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left => write!(f, "Joint"),
            Self::Middle => write!(f, "Unassigned"),
            Self::Right => write!(f, "Personal"),
            Self::Bottom => write!(f, "Ignored"),
        }
    }
}

/// A single row displayed inside a pane: the underlying transaction plus the
/// view-only state needed to render and track it.
///
/// `Item` is a frontend view model; `txn` is the shared data model. Keeping the
/// transaction composed (rather than flattening its fields) means an `Item` is
/// explicitly "a presented transaction" and the projection back out is trivial.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: u32,                     // stable per-session row identity
    pub label: String,               // pre-formatted display string
    pub auto_matched: bool,          // routed here by an auto-assign rule
    pub txn: hho_types::Transaction, // the (override-applied) data
}

impl Item {
    /// The transaction this row represents.
    pub fn to_transaction(&self) -> hho_types::Transaction {
        self.txn.clone()
    }
}

/// Calculates the net total sum of pane items in cents.
/// Sums credit amounts as positive values and debit amounts as negative values.
pub fn calculate_total_cents(items: &[Item]) -> i64 {
    items
        .iter()
        .map(|item| hho_types::net_cents(item.txn.amount_cents, item.txn.direction))
        .sum()
}

/// Compile a user rule pattern into an anchored, whole-string regex.
/// Wrapping in `^(?:…)$` here (in one place) keeps the rule-editor's match
/// preview and the filter that applies rules in exact agreement.
pub fn compile_rule(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(&format!("^(?:{})$", pattern))
}

// ── Row navigation ───────────────────────────────────────────────────────────

/// Compute new selection index after pressing ArrowUp.
/// Clamps at 0; returns `None` for an empty list.
pub fn nav_up(items: &[Item], sel: Option<usize>) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    Some(sel.map_or(0, |i| i.saturating_sub(1)))
}

/// Compute new selection index after pressing ArrowDown.
/// Clamps at the last index; returns `None` for an empty list.
pub fn nav_down(items: &[Item], sel: Option<usize>) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let last = items.len() - 1;
    Some(sel.map_or(0, |i| (i + 1).min(last)))
}

// ── Item transfer ────────────────────────────────────────────────────────────

/// Move the item at `sel` from `source` to `dest`, maintaining chronological order by date in `dest`.
///
/// Returns `(new_source, new_dest, new_source_sel)`.
/// No-op when `sel` is `None` or out of range — returns inputs unchanged.
/// After removal, source selection becomes `min(idx, source.len() - 1)`,
/// or `None` when source is now empty.
pub fn transfer_item(
    mut source: Vec<Item>,
    mut dest: Vec<Item>,
    sel: Option<usize>,
) -> (Vec<Item>, Vec<Item>, Option<usize>) {
    let Some(idx) = sel else {
        return (source, dest, sel);
    };
    if idx >= source.len() {
        return (source, dest, sel);
    }
    let item = source.remove(idx);
    dest.push(item);
    // Sorts destination pane items in date order from oldest to youngest.
    dest.sort_by(|a, b| a.txn.date.cmp(&b.txn.date));
    let new_sel = if source.is_empty() {
        None
    } else {
        Some(idx.min(source.len() - 1))
    };
    (source, dest, new_sel)
}

// ── Pane switching ───────────────────────────────────────────────────────────

/// Target pane for ArrowLeft: Right→Middle→Left→Bottom→Left rotation.
pub fn pane_left(current: ActivePane) -> ActivePane {
    match current {
        ActivePane::Left => ActivePane::Bottom,
        ActivePane::Middle => ActivePane::Left,
        ActivePane::Right => ActivePane::Middle,
        ActivePane::Bottom => ActivePane::Left,
    }
}

/// Target pane for ArrowRight: Left→Middle→Right→Bottom→Right rotation.
pub fn pane_right(current: ActivePane) -> ActivePane {
    match current {
        ActivePane::Left => ActivePane::Middle,
        ActivePane::Middle => ActivePane::Right,
        ActivePane::Right => ActivePane::Bottom,
        ActivePane::Bottom => ActivePane::Right,
    }
}

// ── Item ID generation ───────────────────────────────────────────────────────

use std::cell::Cell;

thread_local! {
    // Monotonically increasing counter; wraps on overflow (u32::MAX items unlikely).
    static ITEM_ID_COUNTER: Cell<u32> = const { Cell::new(1) };
}

pub fn next_item_id() -> u32 {
    ITEM_ID_COUNTER.with(|c| {
        let id = c.get();
        c.set(id.wrapping_add(1));
        id
    })
}

/// Matches a transaction date string against a selected year and month.
/// Expects date format "YYYY-MM-DD".
pub fn match_month_year(date_str: &str, year: i32, month: i32) -> bool {
    if date_str.len() < 10 {
        return false;
    }
    let t_year: i32 = date_str[0..4].parse().unwrap_or(0);
    let t_month: i32 = date_str[5..7].parse().unwrap_or(0);
    t_year == year && t_month == month
}

/// Array of 1-based month numbers and their abbreviations.
pub const MONTHS_ABBR: [(i32, &str); 12] = [
    (1, "Jan"),
    (2, "Feb"),
    (3, "Mar"),
    (4, "Apr"),
    (5, "May"),
    (6, "Jun"),
    (7, "Jul"),
    (8, "Aug"),
    (9, "Sep"),
    (10, "Oct"),
    (11, "Nov"),
    (12, "Dec"),
];

/// Translates month indices (1-12) into abbreviated English month name strings.
pub fn get_month_abbr(month: i32) -> &'static str {
    if (1..=12).contains(&month) {
        MONTHS_ABBR[(month - 1) as usize].1
    } else {
        ""
    }
}

/// Translates month indices (1-12) into full English month name strings.
pub fn get_month_name(month: i32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

/// Calculates the previous calendar month and year.
/// Accepts year and 1-based month. Returns (prev_year, prev_month).
pub fn get_previous_month_year(year: i32, month: i32) -> (i32, i32) {
    if month <= 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

/// Escapes regular expression special characters in the input string.
pub fn escape_regex(input: &str) -> String {
    regex::escape(input)
}

/// Formats a transaction as a single-line pane label.
/// Indicates debit flow with a leading negative sign and credit flow with a leading positive sign.
/// Always returns a five-column layout containing date, vendor, description, amount, and category.
pub fn format_txn(t: &hho_types::Transaction) -> String {
    let amount =
        hho_types::format_dollars_signed(hho_types::net_cents(t.amount_cents, t.direction));
    format!(
        "{} │ {} │ {} │ {} │ {}",
        t.date, t.vendor, t.description, amount, t.category
    )
}

/// A compiled cache of an auto-assign rule for fast/repeated matching.
pub struct CompiledRule {
    pub vendor_re: Option<regex::Regex>,
    pub desc_re: Option<regex::Regex>,
    pub pane: hho_types::RulePane,
    pub category_override: Option<String>,
}

impl CompiledRule {
    /// Compiles a rule, returning None if the pattern contains errors or matches nothing.
    pub fn new(rule: &hho_types::AutoAssignRule) -> Option<Self> {
        let v_pat = rule.vendor_pattern().filter(|s| !s.is_empty());
        let d_pat = rule.description_pattern().filter(|s| !s.is_empty());

        if v_pat.is_none() && d_pat.is_none() {
            return None;
        }

        let vendor_re = match v_pat {
            Some(pat) => Some(compile_rule(pat).ok()?),
            None => None,
        };
        let desc_re = match d_pat {
            Some(pat) => Some(compile_rule(pat).ok()?),
            None => None,
        };

        Some(Self {
            vendor_re,
            desc_re,
            pane: rule.pane,
            category_override: rule.category_override.clone(),
        })
    }

    /// Evaluates if a transaction's vendor and description match this rule.
    pub fn matches(&self, vendor: &str, description: &str) -> bool {
        let matches_vendor = match &self.vendor_re {
            Some(re) => re.is_match(vendor),
            None => true,
        };
        let matches_desc = match &self.desc_re {
            Some(re) => re.is_match(description),
            None => true,
        };
        matches_vendor && matches_desc
    }
}

/// Routes transactions into destination panes based on auto-assign rules.
/// Returns a tuple of item lists corresponding to the Left (Joint), Middle (Unassigned),
/// Right (Personal), and Bottom (Ignored) panes respectively.
pub fn classify_transactions(
    txns: Vec<hho_types::Transaction>,
    rules: &[hho_types::AutoAssignRule],
) -> (Vec<Item>, Vec<Item>, Vec<Item>, Vec<Item>) {
    let compiled_rules: Vec<CompiledRule> = rules
        .iter()
        .filter_map(CompiledRule::new)
        .collect();

    let mut left = vec![];
    let mut middle = vec![];
    let mut right = vec![];
    let mut bottom = vec![];

    for t in txns {
        let mut matched_pane = None;
        let mut overridden_category = None;
        let mut is_manual = false;

        if let Some(pane) = t.manual_pane {
            matched_pane = Some(pane);
            is_manual = true;
        } else {
            for rule in &compiled_rules {
                if rule.matches(&t.vendor, &t.description) {
                    matched_pane = Some(rule.pane);
                    overridden_category = rule.category_override.clone();
                    break;
                }
            }
        }

        let mut row_cols = t.row_cols.clone();
        let category = if let Some(ref cat) = overridden_category {
            if let Some(ref mut cols) = row_cols {
                if let Some(idx) = t.category_col {
                    if idx < cols.len() {
                        cols[idx] = cat.clone();
                    }
                }
            }
            cat.clone()
        } else {
            t.category.clone()
        };

        let txn = hho_types::Transaction {
            id: t.id,
            date: t.date.clone(),
            vendor: t.vendor.clone(),
            description: t.description.clone(),
            category,
            amount_cents: t.amount_cents,
            direction: t.direction,
            manual_pane: t.manual_pane,
            row_cols,
            date_col: t.date_col,
            vendor_col: t.vendor_col,
            description_col: t.description_col,
            category_col: t.category_col,
            amount_col: t.amount_col,
        };
        let item = Item {
            id: next_item_id(),
            label: format_txn(&txn),
            auto_matched: matched_pane.is_some() && !is_manual,
            txn,
        };

        match matched_pane {
            Some(hho_types::RulePane::Joint) => left.push(item),
            Some(hho_types::RulePane::Personal) => right.push(item),
            Some(hho_types::RulePane::Ignored) => bottom.push(item),
            Some(hho_types::RulePane::Unassigned) | None => middle.push(item),
        }
    }

    (left, middle, right, bottom)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn items(labels: &[&str]) -> Vec<Item> {
        labels
            .iter()
            .enumerate()
            .map(|(i, l)| Item {
                id: i as u32,
                label: l.to_string(),
                auto_matched: false,
                txn: hho_types::Transaction {
                    id: None,
                    date: "".to_string(),
                    vendor: "".to_string(),
                    category: "".to_string(),
                    amount_cents: 0,
                    direction: hho_types::Direction::Debit,
                    manual_pane: None,
                    ..Default::default()
                },
            })
            .collect()
    }

    // nav_up ──────────────────────────────────────────────────────────────────

    #[test]
    fn nav_up_from_middle_row_moves_to_previous() {
        assert_eq!(nav_up(&items(&["a", "b", "c"]), Some(2)), Some(1));
    }

    #[test]
    fn nav_up_at_top_row_clamps_to_zero() {
        assert_eq!(nav_up(&items(&["a", "b", "c"]), Some(0)), Some(0));
    }

    #[test]
    fn nav_up_with_no_selection_selects_first_row() {
        assert_eq!(nav_up(&items(&["a", "b"]), None), Some(0));
    }

    #[test]
    fn nav_up_on_empty_list_returns_none() {
        assert_eq!(nav_up(&[], None), None);
        assert_eq!(nav_up(&[], Some(0)), None);
    }

    // nav_down ────────────────────────────────────────────────────────────────

    #[test]
    fn nav_down_from_first_row_moves_to_next() {
        assert_eq!(nav_down(&items(&["a", "b", "c"]), Some(0)), Some(1));
    }

    #[test]
    fn nav_down_at_last_row_clamps_to_last() {
        assert_eq!(nav_down(&items(&["a", "b", "c"]), Some(2)), Some(2));
    }

    #[test]
    fn nav_down_with_no_selection_selects_first_row() {
        assert_eq!(nav_down(&items(&["a", "b"]), None), Some(0));
    }

    #[test]
    fn nav_down_on_empty_list_returns_none() {
        assert_eq!(nav_down(&[], None), None);
    }

    // transfer_item ───────────────────────────────────────────────────────────

    #[test]
    fn transfer_item_moves_selected_to_end_of_dest() {
        let (new_src, new_dst, new_sel) =
            transfer_item(items(&["a", "b", "c"]), items(&["x"]), Some(1));
        let src_labels: Vec<_> = new_src.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(src_labels, ["a", "c"]);
        assert_eq!(new_dst.last().unwrap().label, "b");
        assert_eq!(new_sel, Some(1));
    }

    #[test]
    fn transfer_item_last_item_clears_source_selection() {
        let (new_src, new_dst, new_sel) = transfer_item(items(&["a"]), items(&[]), Some(0));
        assert!(new_src.is_empty());
        assert_eq!(new_dst.len(), 1);
        assert_eq!(new_sel, None);
    }

    #[test]
    fn transfer_item_sel_clamps_when_removing_last_element() {
        let (_, _, new_sel) = transfer_item(items(&["a", "b", "c"]), items(&[]), Some(2));
        // Removed index 2 (last); new last is index 1.
        assert_eq!(new_sel, Some(1));
    }

    #[test]
    fn transfer_item_noop_when_sel_is_none() {
        let (new_src, new_dst, new_sel) = transfer_item(items(&["a", "b"]), items(&[]), None);
        assert_eq!(new_src.len(), 2);
        assert_eq!(new_dst.len(), 0);
        assert_eq!(new_sel, None);
    }

    #[test]
    fn transfer_item_noop_when_sel_out_of_range() {
        let (new_src, new_dst, new_sel) = transfer_item(items(&["a"]), items(&[]), Some(5));
        assert_eq!(new_src.len(), 1);
        assert_eq!(new_dst.len(), 0);
        assert_eq!(new_sel, Some(5));
    }

    #[test]
    fn transfer_item_preserves_remaining_item_order() {
        let (new_src, _, _) = transfer_item(items(&["a", "b", "c", "d"]), items(&[]), Some(1));
        let labels: Vec<_> = new_src.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["a", "c", "d"]);
    }

    // next_item_id ────────────────────────────────────────────────────────────

    #[test]
    fn next_item_id_increments_monotonically() {
        let a = next_item_id();
        let b = next_item_id();
        assert!(b > a, "expected {b} > {a}");
    }

    // pane_left ───────────────────────────────────────────────────────────────

    #[test]
    fn pane_left_from_middle_goes_to_left() {
        assert_eq!(pane_left(ActivePane::Middle), ActivePane::Left);
    }

    #[test]
    fn pane_left_from_right_goes_to_middle() {
        assert_eq!(pane_left(ActivePane::Right), ActivePane::Middle);
    }

    #[test]
    fn pane_left_from_left_goes_to_bottom() {
        assert_eq!(pane_left(ActivePane::Left), ActivePane::Bottom);
    }

    #[test]
    fn pane_left_from_bottom_goes_to_left() {
        assert_eq!(pane_left(ActivePane::Bottom), ActivePane::Left);
    }

    // pane_right ──────────────────────────────────────────────────────────────

    #[test]
    fn pane_right_from_left_goes_to_middle() {
        assert_eq!(pane_right(ActivePane::Left), ActivePane::Middle);
    }

    #[test]
    fn pane_right_from_middle_goes_to_right() {
        assert_eq!(pane_right(ActivePane::Middle), ActivePane::Right);
    }

    #[test]
    fn pane_right_from_right_goes_to_bottom() {
        assert_eq!(pane_right(ActivePane::Right), ActivePane::Bottom);
    }

    #[test]
    fn pane_right_from_bottom_goes_to_right() {
        assert_eq!(pane_right(ActivePane::Bottom), ActivePane::Right);
    }

    // Month / Year filtering tests

    #[test]
    fn match_month_year_validates_dates_correctly() {
        assert!(match_month_year("2026-05-18", 2026, 5));
        assert!(match_month_year("2025-12-01", 2025, 12));
        assert!(!match_month_year("2026-05-18", 2026, 6));
        assert!(!match_month_year("2026-05-18", 2025, 5));
        assert!(!match_month_year("invalid", 2026, 5));
        assert!(!match_month_year("2026-05", 2026, 5));
    }

    #[test]
    fn get_previous_month_year_calculates_correct_periods() {
        assert_eq!(get_previous_month_year(2026, 5), (2026, 4));
        assert_eq!(get_previous_month_year(2026, 1), (2025, 12));
    }

    #[test]
    fn calculate_total_cents_sums_credits_and_debits() {
        let items = vec![
            Item {
                id: 1,
                label: "a".to_string(),
                auto_matched: false,
                txn: hho_types::Transaction {
                    id: None,
                    date: "".to_string(),
                    vendor: "".to_string(),
                    category: "".to_string(),
                    amount_cents: 1000,
                    direction: hho_types::Direction::Credit,
                    manual_pane: None,
                    ..Default::default()
                },
            },
            Item {
                id: 2,
                label: "b".to_string(),
                auto_matched: false,
                txn: hho_types::Transaction {
                    id: None,
                    date: "".to_string(),
                    vendor: "".to_string(),
                    category: "".to_string(),
                    amount_cents: 250,
                    direction: hho_types::Direction::Debit,
                    manual_pane: None,
                    ..Default::default()
                },
            },
        ];
        assert_eq!(calculate_total_cents(&items), 750);
    }

    #[test]
    fn transfer_item_keeps_dest_sorted_by_date() {
        let source = vec![Item {
            id: 1,
            label: "2026-05-18 │ a".into(),
            auto_matched: false,
            txn: hho_types::Transaction {
                id: None,
                date: "2026-05-18".into(),
                vendor: "".to_string(),
                category: "".to_string(),
                amount_cents: 100,
                direction: hho_types::Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
        }];
        let dest = vec![Item {
            id: 2,
            label: "2026-05-20 │ b".into(),
            auto_matched: false,
            txn: hho_types::Transaction {
                id: None,
                date: "2026-05-20".into(),
                vendor: "".to_string(),
                category: "".to_string(),
                amount_cents: 200,
                direction: hho_types::Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
        }];
        let (_, new_dst, _) = transfer_item(source, dest, Some(0));
        assert_eq!(new_dst[0].txn.date, "2026-05-18");
        assert_eq!(new_dst[1].txn.date, "2026-05-20");
    }

    #[test]
    fn compile_rule_anchors_whole_string_match() {
        let re = compile_rule("STAR.*").unwrap();
        assert!(re.is_match("STARBUCKS"));
        assert!(!re.is_match("MORNINGSTAR")); // anchored: must match from the start
        let re2 = compile_rule("BUCKS").unwrap();
        assert!(!re2.is_match("STARBUCKS")); // anchored: must match the whole string
        assert!(compile_rule("(unclosed").is_err());
    }

    #[test]
    fn test_escape_regex_escapes_special_characters() {
        assert_eq!(escape_regex("Google.com"), "Google\\.com");
        assert_eq!(escape_regex("Shop*"), "Shop\\*");
        assert_eq!(escape_regex("Vendor (US)"), "Vendor \\(US\\)");
    }

    #[test]
    fn test_classify_transactions_routes_correctly() {
        use hho_types::{AutoAssignRule, Direction, RulePane, Transaction};

        let txns = vec![
            Transaction {
                id: None,
                date: "2026-05-15".to_string(),
                vendor: "STARBUCKS COFFEE".to_string(),
                category: "Uncategorized".to_string(),
                amount_cents: 450,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-16".to_string(),
                vendor: "NETFLIX".to_string(),
                category: "Entertainment".to_string(),
                amount_cents: 1599,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-17".to_string(),
                vendor: "SAFEWAY".to_string(),
                category: "Groceries".to_string(),
                amount_cents: 5000,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-18".to_string(),
                vendor: "SPAMMY_EMAIL".to_string(),
                category: "Misc".to_string(),
                amount_cents: 100,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
        ];

        let rules = vec![
            AutoAssignRule {
                regex: Some("STARBUCKS.*".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Joint,
                category_override: Some("Coffee & Tea".to_string()),
            },
            AutoAssignRule {
                regex: Some("NETFLIX".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Personal,
                category_override: None,
            },
            AutoAssignRule {
                regex: Some("SPAMMY_EMAIL".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Ignored,
                category_override: Some("Junk".to_string()),
            },
        ];

        let (left, middle, right, bottom) = classify_transactions(txns, &rules);

        // Verify Starbucks matches the rule and goes to Joint (left) with overridden category
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].txn.vendor, "STARBUCKS COFFEE");
        assert_eq!(left[0].txn.category, "Coffee & Tea");
        assert!(left[0].auto_matched);

        // Verify Netflix matches the rule and goes to Personal (right) with original category
        assert_eq!(right.len(), 1);
        assert_eq!(right[0].txn.vendor, "NETFLIX");
        assert_eq!(right[0].txn.category, "Entertainment");
        assert!(right[0].auto_matched);

        // Verify Spammy email matches the rule and goes to Ignored (bottom) with overridden category
        assert_eq!(bottom.len(), 1);
        assert_eq!(bottom[0].txn.vendor, "SPAMMY_EMAIL");
        assert_eq!(bottom[0].txn.category, "Junk");
        assert!(bottom[0].auto_matched);

        // Verify Safeway does not match rules and goes to Unassigned (middle)
        assert_eq!(middle.len(), 1);
        assert_eq!(middle[0].txn.vendor, "SAFEWAY");
        assert_eq!(middle[0].txn.category, "Groceries");
        assert!(!middle[0].auto_matched);
    }

    #[test]
    fn test_month_names_logic() {
        assert_eq!(get_month_name(1), "January");
        assert_eq!(get_month_name(12), "December");
        assert_eq!(get_month_name(13), "Unknown");
        assert_eq!(get_month_name(0), "Unknown");

        assert_eq!(get_month_abbr(1), "Jan");
        assert_eq!(get_month_abbr(12), "Dec");
        assert_eq!(get_month_abbr(13), "");
        assert_eq!(get_month_abbr(0), "");

        assert_eq!(MONTHS_ABBR.len(), 12);
        assert_eq!(MONTHS_ABBR[0], (1, "Jan"));
        assert_eq!(MONTHS_ABBR[11], (12, "Dec"));
    }

    #[test]
    fn test_pane_title_alignment() {
        // Enforce that display titles for equivalent active and rule panes match.
        assert_eq!(ActivePane::Left.to_string(), hho_types::RulePane::Joint.display_title());
        assert_eq!(ActivePane::Right.to_string(), hho_types::RulePane::Personal.display_title());
        assert_eq!(ActivePane::Bottom.to_string(), hho_types::RulePane::Ignored.display_title());
    }

    #[test]
    fn test_classify_transactions_first_match_wins() {
        use hho_types::{AutoAssignRule, Direction, RulePane, Transaction};

        // Enforce that the first matching rule takes precedence.
        let txns = vec![Transaction {
            id: None,
            date: "2026-05-15".to_string(),
            vendor: "STARBUCKS COFFEE".to_string(),
            category: "Uncategorized".to_string(),
            amount_cents: 450,
            direction: Direction::Debit,
            manual_pane: None,
            ..Default::default()
        }];

        let rules = vec![
            AutoAssignRule {
                regex: Some("STARBUCKS.*".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Joint,
                category_override: Some("Coffee".to_string()),
            },
            AutoAssignRule {
                regex: Some(".*COFFEE".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Personal,
                category_override: Some("Tea".to_string()),
            },
        ];

        let (left, middle, right, bottom) = classify_transactions(txns, &rules);

        assert_eq!(left.len(), 1);
        assert_eq!(left[0].txn.category, "Coffee");
        assert!(right.is_empty());
        assert!(middle.is_empty());
        assert!(bottom.is_empty());
    }

    #[test]
    fn test_classify_transactions_with_multi_regex_matching() {
        use hho_types::{AutoAssignRule, Direction, RulePane, Transaction};

        let txns = vec![
            Transaction {
                id: None,
                date: "2026-05-15".to_string(),
                vendor: "STARBUCKS COFFEE".to_string(),
                description: "Seattle branch".to_string(),
                category: "Uncategorized".to_string(),
                amount_cents: 450,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-16".to_string(),
                vendor: "NETFLIX".to_string(),
                description: "Monthly subscription fee".to_string(),
                category: "Entertainment".to_string(),
                amount_cents: 1599,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-17".to_string(),
                vendor: "SAFEWAY".to_string(),
                description: "Food supplies".to_string(),
                category: "Groceries".to_string(),
                amount_cents: 5000,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
        ];

        // Rules to test:
        // Rule 1: Matches Vendor only ("STARBUCKS.*") -> Joint
        // Rule 2: Matches BOTH Vendor and Description ("NETFLIX" & "Monthly.*") -> Personal
        // Rule 3: Matches Description only ("Food.*") -> Ignored
        // Rule 4: Matches BOTH Vendor and Description, but one mismatch -> shouldn't match (remains in Unassigned)
        let rules = vec![
            AutoAssignRule {
                regex: None,
                vendor_regex: Some("STARBUCKS.*".to_string()),
                description_regex: None,
                pane: RulePane::Joint,
                category_override: None,
            },
            AutoAssignRule {
                regex: None,
                vendor_regex: Some("NETFLIX".to_string()),
                description_regex: Some("Monthly.*".to_string()),
                pane: RulePane::Personal,
                category_override: None,
            },
            AutoAssignRule {
                regex: None,
                vendor_regex: None,
                description_regex: Some("Food.*".to_string()),
                pane: RulePane::Ignored,
                category_override: None,
            },
            AutoAssignRule {
                regex: None,
                vendor_regex: Some("SAFEWAY".to_string()),
                description_regex: Some("Gasoline.*".to_string()), // Doesn't match "Food supplies"
                pane: RulePane::Joint,
                category_override: None,
            },
        ];

        let (left, middle, right, bottom) = classify_transactions(txns, &rules);

        // Starbucks matched Rule 1 (Vendor only) and goes to Joint (left)
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].txn.vendor, "STARBUCKS COFFEE");

        // Netflix matched Rule 2 (Both match) and goes to Personal (right)
        assert_eq!(right.len(), 1);
        assert_eq!(right[0].txn.vendor, "NETFLIX");

        // Safeway:
        // - Matches Description only rule (Rule 3) and goes to Ignored (bottom)
        // - Rule 4 fails to match because description mismatch ("Gasoline.*" vs "Food supplies")
        // Since Rule 3 comes first and matches description, it should go to Ignored!
        assert_eq!(bottom.len(), 1);
        assert_eq!(bottom[0].txn.vendor, "SAFEWAY");

        // Unassigned middle should be empty since all 3 matched a rule
        assert!(middle.is_empty());
    }

    #[test]
    fn test_format_txn_output() {
        use hho_types::{Direction, Transaction};

        // Construct transaction with populated description.
        let txn_with_desc = Transaction {
            id: None,
            date: "2026-05-15".to_string(),
            vendor: "STARBUCKS".to_string(),
            description: "Seattle branch".to_string(),
            category: "Coffee".to_string(),
            amount_cents: 450,
            direction: Direction::Debit,
            ..Default::default()
        };

        // Construct transaction with empty description.
        let txn_no_desc = Transaction {
            id: None,
            date: "2026-05-16".to_string(),
            vendor: "NETFLIX".to_string(),
            description: "".to_string(),
            category: "Streaming".to_string(),
            amount_cents: 1599,
            direction: Direction::Debit,
            ..Default::default()
        };

        // Assert formatted output containing description.
        let label1 = format_txn(&txn_with_desc);
        assert_eq!(label1, "2026-05-15 │ STARBUCKS │ Seattle branch │ -$4.50 │ Coffee");

        // Assert formatted output containing empty description column.
        let label2 = format_txn(&txn_no_desc);
        assert_eq!(label2, "2026-05-16 │ NETFLIX │  │ -$15.99 │ Streaming");
    }
}
