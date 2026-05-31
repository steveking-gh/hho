// Shared IPC contract between the hho frontend (WASM/Leptos) and backend (Tauri).
//
// Every type that crosses the command boundary lives here exactly once, so the
// two separately-compiled crates cannot drift: a renamed field or changed type
// becomes a compile error on whichever side is stale.
//
// Argument structs carry `#[serde(rename_all = "camelCase")]` because Tauri v2
// matches command arguments by camelCase keys.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ── Domain types ──────────────────────────────────────────────────────────────

/// Direction of money flow for a transaction.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Debit,  // money out
    Credit, // money in
}

/// How an institution encodes amount magnitude and debit/credit direction.
/// Internally tagged on `amount_scheme` and flattened into Institution so the
/// persisted TOML and the IPC JSON both stay flat and readable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "amount_scheme", rename_all = "snake_case")]
pub enum AmountScheme {
    /// One signed amount column; sign determines direction.
    SingleSigned {
        amount_col: usize,
        debit_is_negative: bool,
    },
    /// Magnitude in one column; a separate text column labels the direction.
    TypeColumn {
        amount_col: usize,
        type_col: usize,
        debit_labels: Vec<String>,
        credit_labels: Vec<String>,
    },
}

/// A saved per-institution column mapping, keyed by header fingerprint.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Institution {
    pub name: String,
    pub fingerprint: String,
    pub date_col: usize,
    pub vendor_col: usize,
    #[serde(default)]
    pub category_col: Option<usize>,
    #[serde(default)]
    pub ignore_cols: Vec<usize>,
    #[serde(flatten)]
    pub amount: AmountScheme,
}

/// A normalized transaction produced by applying an Institution to a CSV row.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Transaction {
    pub date: String, // canonical "YYYY-MM-DD"
    pub vendor: String,
    pub category: String,
    pub amount_cents: i64, // magnitude, always >= 0
    pub direction: Direction,
}

/// Identifies which amount scheme parsing strategy applies.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AmountSchemeTag {
    SingleSigned,
    TypeColumn,
}

impl std::fmt::Display for AmountSchemeTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SingleSigned => write!(f, "single_signed"),
            Self::TypeColumn => write!(f, "type_column"),
        }
    }
}

/// Heuristic mapping suggestion shown as the modal's initial state.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SuggestedMapping {
    pub date_col: usize,
    pub vendor_col: usize,
    pub amount_col: usize,
    pub type_col: Option<usize>,
    pub category_col: Option<usize>,
    pub scheme: AmountSchemeTag,
    pub debit_is_negative: bool,
    pub ignore_cols: Vec<usize>,
}

// ── Command results ───────────────────────────────────────────────────────────

/// Outcome of opening a CSV.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum OpenResult {
    Mapped {
        institution: String,
        transactions: Vec<Transaction>,
    },
    NeedsMapping {
        fingerprint: String,
        headers: Vec<String>,
        sample_rows: Vec<Vec<String>>,
        pending_path: String,
        suggested: SuggestedMapping,
    },
    Cancelled,
}

/// Persisted pane dimensions returned by `get_layout`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LayoutConfig {
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_h: f32,
    pub debug_h: f32,
}

// ── Money helpers ─────────────────────────────────────────────────────────────
// Single source of truth for cents arithmetic and currency formatting, shared by
// the frontend pane headers / row labels and the backend CSV export.

/// Signed net cents for an amount: credits positive, debits negative.
/// `amount_cents` is always a non-negative magnitude.
pub fn net_cents(amount_cents: i64, direction: Direction) -> i64 {
    match direction {
        Direction::Credit => amount_cents,
        Direction::Debit => -amount_cents,
    }
}

/// Format signed cents without a currency symbol: `-28697 → "-286.97"`,
/// `1000 → "10.00"`. Used for CSV output.
pub fn format_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{}{}.{:02}", sign, abs / 100, abs % 100)
}

/// Format signed cents as currency, showing "-" only when negative:
/// `-540 → "-$5.40"`, `540 → "$5.40"`. Used for pane-header totals.
pub fn format_dollars(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{}${}.{:02}", sign, abs / 100, abs % 100)
}

/// Format signed cents as currency with an explicit sign:
/// `540 → "+$5.40"`, `-540 → "-$5.40"`. Used for per-transaction row labels.
pub fn format_dollars_signed(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "+" };
    let abs = cents.abs();
    format!("{}${}.{:02}", sign, abs / 100, abs % 100)
}

/// Parses a currency string into signed integer cents.
/// Handles `$`, thousands commas, leading sign, `(1.23)` parentheses-negatives,
/// and trailing `CR`/`DR` markers. Truncates beyond two decimal places.
pub fn parse_amount_cents(raw: &str) -> Option<i64> {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return None;
    }

    let mut negative = false;

    // Parentheses denote a negative amount.
    if s.starts_with('(') && s.ends_with(')') {
        negative = true;
        s = s[1..s.len() - 1].to_string();
    }

    // Trailing CR (credit/positive) or DR (debit/negative) markers.
    let upper = s.to_uppercase();
    if upper.ends_with("CR") {
        s.truncate(s.len() - 2);
    } else if upper.ends_with("DR") {
        s.truncate(s.len() - 2);
        negative = true;
    }

    // Keep only digits, decimal point, and minus; drop $, commas, spaces.
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();

    // A leading minus flips the sign; reject any other stray minus later.
    let body = match cleaned.strip_prefix('-') {
        Some(rest) => {
            negative = !negative;
            rest.to_string()
        }
        None => cleaned,
    };
    if body.is_empty() {
        return None;
    }

    // Split integer and fractional parts on a single decimal point.
    let mut parts = body.split('.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return None; // more than one decimal point
    }

    let int_val: i64 = if int_part.is_empty() {
        0
    } else {
        int_part.parse().ok()?
    };
    let frac_cents: i64 = match frac_part.len() {
        0 => 0,
        1 => frac_part.parse::<i64>().ok()? * 10,
        _ => frac_part[..2].parse::<i64>().ok()?, // truncate beyond 2 digits
    };

    let cents = int_val * 100 + frac_cents;
    Some(if negative { -cents } else { cents })
}

/// Net cents per category, sorted by name. Blank/whitespace categories collapse
/// to "(No Category)". Each entry is `(category, net_cents)`.
pub fn summarize_by_category<'a, I>(entries: I) -> BTreeMap<String, i64>
where
    I: IntoIterator<Item = (&'a str, i64)>,
{
    let mut map = BTreeMap::new();
    for (cat, net) in entries {
        let name = if cat.trim().is_empty() {
            "(No Category)".to_string()
        } else {
            cat.trim().to_string()
        };
        *map.entry(name).or_insert(0i64) += net;
    }
    map
}

// ── Command argument structs (frontend → backend) ─────────────────────────────

/// Arguments for the `open_csv` command.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpenCsvArgs {
    pub path: String,
}

/// Arguments for the `save_mapping` command.
/// `pending_path` serializes as `pendingPath`; the nested `institution` keeps
/// its own (snake_case) field names, which the backend deserializes directly.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveMappingArgs {
    pub institution: Institution,
    pub pending_path: String,
}

/// Arguments for the `save_layout` command.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveLayoutArgs {
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_h: f32,
    pub debug_h: f32,
}

/// Arguments for the `save_window_size` command.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveWindowSizeArgs {
    pub width: f64,
    pub height: f64,
}

/// Target pane destination for automatic transaction assignment.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RulePane {
    Joint,
    Personal,
    Ignored,
}

impl std::fmt::Display for RulePane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Joint => write!(f, "joint"),
            Self::Personal => write!(f, "personal"),
            Self::Ignored => write!(f, "ignored"),
        }
    }
}

/// A regex rule mapping a vendor name to a destination pane.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AutoAssignRule {
    pub regex: String,
    pub pane: RulePane,
    #[serde(default)]
    pub category_override: Option<String>,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_cents_signs_by_direction() {
        assert_eq!(net_cents(540, Direction::Credit), 540);
        assert_eq!(net_cents(540, Direction::Debit), -540);
    }

    #[test]
    fn format_cents_has_no_symbol() {
        assert_eq!(format_cents(-28697), "-286.97");
        assert_eq!(format_cents(1000), "10.00");
        assert_eq!(format_cents(5), "0.05");
        assert_eq!(format_cents(0), "0.00");
    }

    #[test]
    fn format_dollars_shows_minus_only() {
        assert_eq!(format_dollars(-540), "-$5.40");
        assert_eq!(format_dollars(540), "$5.40");
        assert_eq!(format_dollars(0), "$0.00");
    }

    #[test]
    fn format_dollars_signed_always_shows_sign() {
        assert_eq!(format_dollars_signed(540), "+$5.40");
        assert_eq!(format_dollars_signed(-540), "-$5.40");
    }

    #[test]
    fn summarize_by_category_nets_and_labels_blanks() {
        let entries = vec![
            ("Travel", -28697),
            ("", -540),       // blank → "(No Category)"
            ("  ", 100),      // whitespace also collapses, and sums in
            ("Travel", -100), // same category accumulates
        ];
        let summary = summarize_by_category(entries);
        assert_eq!(summary.get("Travel"), Some(&-28797));
        assert_eq!(summary.get("(No Category)"), Some(&-440));
        // BTreeMap iteration is sorted: "(No Category)" precedes "Travel".
        let keys: Vec<_> = summary.keys().cloned().collect();
        assert_eq!(
            keys,
            vec!["(No Category)".to_string(), "Travel".to_string()]
        );
    }

    #[test]
    fn test_parse_amount_cents() {
        assert_eq!(parse_amount_cents("286.97"), Some(28697));
        assert_eq!(parse_amount_cents("-286.97"), Some(-28697));
        assert_eq!(parse_amount_cents("3367.17"), Some(336717));
        assert_eq!(parse_amount_cents("$1,234.56"), Some(123456));
        assert_eq!(parse_amount_cents("-$1,234.56"), Some(-123456));
        assert_eq!(parse_amount_cents("(5.40)"), Some(-540));
        assert_eq!(parse_amount_cents("($5.40)"), Some(-540));
        assert_eq!(parse_amount_cents("5.40 CR"), Some(540));
        assert_eq!(parse_amount_cents("5.40 DR"), Some(-540));
        assert_eq!(parse_amount_cents("5"), Some(500));
        assert_eq!(parse_amount_cents("5.4"), Some(540));
        assert_eq!(parse_amount_cents("5.409"), Some(540));
        assert_eq!(parse_amount_cents(""), None);
        assert_eq!(parse_amount_cents("  "), None);
        assert_eq!(parse_amount_cents("abc"), None);
        assert_eq!(parse_amount_cents("1.2.3"), None);
    }
}
