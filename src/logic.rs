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
            Self::Left   => write!(f, "Joint"),
            Self::Middle => write!(f, "Unassigned"),
            Self::Right  => write!(f, "Personal"),
            Self::Bottom => write!(f, "Ignored"),
        }
    }
}

/// A single row entry displayed inside a pane.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id:           u32,
    pub label:        String,
    pub amount_cents: i64,
    pub direction:    hho_types::Direction,
    pub date:         String,
    pub auto_matched: bool,
}

/// Calculates the net total sum of pane items in cents.
/// Sums credit amounts as positive values and debit amounts as negative values.
pub fn calculate_total_cents(items: &[Item]) -> i64 {
    items
        .iter()
        .map(|item| match item.direction {
            hho_types::Direction::Credit => item.amount_cents,
            hho_types::Direction::Debit => -item.amount_cents,
        })
        .sum()
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

/// Move the item at `sel` from `source` to the end of `dest`.
///
/// Returns `(new_source, new_dest, new_source_sel)`.
/// No-op when `sel` is `None` or out of range — returns inputs unchanged.
/// After removal, source selection becomes `min(idx, source.len() - 1)`,
/// or `None` when source is now empty.
pub fn transfer_item(
    mut source: Vec<Item>,
    mut dest:   Vec<Item>,
    sel:        Option<usize>,
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
    dest.sort_by(|a, b| a.date.cmp(&b.date));
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
        ActivePane::Left   => ActivePane::Bottom,
        ActivePane::Middle => ActivePane::Left,
        ActivePane::Right  => ActivePane::Middle,
        ActivePane::Bottom => ActivePane::Left,
    }
}

/// Target pane for ArrowRight: Left→Middle→Right→Bottom→Right rotation.
pub fn pane_right(current: ActivePane) -> ActivePane {
    match current {
        ActivePane::Left   => ActivePane::Middle,
        ActivePane::Middle => ActivePane::Right,
        ActivePane::Right  => ActivePane::Bottom,
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
                amount_cents: 0,
                direction: hho_types::Direction::Debit,
                date: "".to_string(),
                auto_matched: false,
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
        let (new_src, new_dst, new_sel) =
            transfer_item(items(&["a"]), items(&[]), Some(0));
        assert!(new_src.is_empty());
        assert_eq!(new_dst.len(), 1);
        assert_eq!(new_sel, None);
    }

    #[test]
    fn transfer_item_sel_clamps_when_removing_last_element() {
        let (_, _, new_sel) =
            transfer_item(items(&["a", "b", "c"]), items(&[]), Some(2));
        // Removed index 2 (last); new last is index 1.
        assert_eq!(new_sel, Some(1));
    }

    #[test]
    fn transfer_item_noop_when_sel_is_none() {
        let (new_src, new_dst, new_sel) =
            transfer_item(items(&["a", "b"]), items(&[]), None);
        assert_eq!(new_src.len(), 2);
        assert_eq!(new_dst.len(), 0);
        assert_eq!(new_sel, None);
    }

    #[test]
    fn transfer_item_noop_when_sel_out_of_range() {
        let (new_src, new_dst, new_sel) =
            transfer_item(items(&["a"]), items(&[]), Some(5));
        assert_eq!(new_src.len(), 1);
        assert_eq!(new_dst.len(), 0);
        assert_eq!(new_sel, Some(5));
    }

    #[test]
    fn transfer_item_preserves_remaining_item_order() {
        let (new_src, _, _) =
            transfer_item(items(&["a", "b", "c", "d"]), items(&[]), Some(1));
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
                amount_cents: 1000,
                direction: hho_types::Direction::Credit,
                date: "".to_string(),
                auto_matched: false,
            },
            Item {
                id: 2,
                label: "b".to_string(),
                amount_cents: 250,
                direction: hho_types::Direction::Debit,
                date: "".to_string(),
                auto_matched: false,
            },
        ];
        assert_eq!(calculate_total_cents(&items), 750);
    }

    #[test]
    fn transfer_item_keeps_dest_sorted_by_date() {
        let source = vec![
            Item {
                id: 1,
                label: "2026-05-18 │ a".into(),
                amount_cents: 100,
                direction: hho_types::Direction::Debit,
                date: "2026-05-18".into(),
                auto_matched: false,
            }
        ];
        let dest = vec![
            Item {
                id: 2,
                label: "2026-05-20 │ b".into(),
                amount_cents: 200,
                direction: hho_types::Direction::Debit,
                date: "2026-05-20".into(),
                auto_matched: false,
            }
        ];
        let (_, new_dst, _) = transfer_item(source, dest, Some(0));
        assert_eq!(new_dst[0].date, "2026-05-18");
        assert_eq!(new_dst[1].date, "2026-05-20");
    }

    #[test]
    fn test_escape_regex_escapes_special_characters() {
        assert_eq!(escape_regex("Google.com"), "Google\\.com");
        assert_eq!(escape_regex("Shop*"), "Shop\\*");
        assert_eq!(escape_regex("Vendor (US)"), "Vendor \\(US\\)");
    }
}
