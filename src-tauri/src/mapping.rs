// Pure transaction-mapping logic: header fingerprinting, amount/date parsing,
// direction resolution, and per-institution row parsing.
// No Tauri or IO dependencies — fully unit-testable on the native target.

use chrono::{Datelike, NaiveDate};

// Domain types live in the shared `hho-types` crate so the frontend and backend
// cannot drift. This module owns the *logic* that operates on them.
use hho_types::{
    parse_amount_cents, AmountScheme, Direction, Institution, SuggestedMapping, Transaction,
};

// ── Fingerprinting ────────────────────────────────────────────────────────────

/// Normalize a header row into a stable key: trim + lowercase each cell,
/// join with commas. Identical institution layouts yield identical keys.
pub fn fingerprint(headers: &[String]) -> String {
    headers
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect::<Vec<_>>()
        .join(",")
}

/// Find the saved institution whose fingerprint matches `fp`.
pub fn find_institution<'a>(fp: &str, list: &'a [Institution]) -> Option<&'a Institution> {
    list.iter().find(|i| i.fingerprint == fp)
}

// ── Date parsing ──────────────────────────────────────────────────────────────

/// Common institution date formats, tried in order.
const DATE_FORMATS: &[&str] = &[
    "%m/%d/%Y",  // 05/18/2026
    "%m-%d-%Y",  // 05-18-2026
    "%Y-%m-%d",  // 2026-05-18
    "%Y/%m/%d",  // 2026/05/18
    "%d-%b-%Y",  // 18-May-2026
    "%d %b %Y",  // 18 May 2026
    "%b %d, %Y", // May 18, 2026
    "%m/%d/%y",  // 05/18/26
];

/// Parse a date string into canonical "YYYY-MM-DD" by trying each known format.
pub fn parse_date(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    for fmt in DATE_FORMATS {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            // `%Y` greedily matches a 2-digit year (e.g. "26" → year 26).
            // Reject that so the dedicated `%y` format claims it (→ 2026).
            if d.year() < 100 {
                continue;
            }
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    None
}

// ── Direction & row resolution ────────────────────────────────────────────────

/// Resolve amount magnitude and direction for a row under `scheme`.
/// Returns None when a referenced column is missing or unparseable, or when a
/// TypeColumn label matches neither the debit nor credit label list.
pub fn resolve_amount(scheme: &AmountScheme, row: &[String]) -> Option<(i64, Direction)> {
    match scheme {
        AmountScheme::SingleSigned {
            amount_col,
            debit_is_negative,
        } => {
            let signed = parse_amount_cents(row.get(*amount_col)?)?;
            let is_neg = signed < 0;
            // Debit when the sign matches the institution's debit convention.
            let direction = if is_neg == *debit_is_negative {
                Direction::Debit
            } else {
                Direction::Credit
            };
            Some((signed.abs(), direction))
        }
        AmountScheme::TypeColumn {
            amount_col,
            type_col,
            debit_labels,
            credit_labels,
        } => {
            let magnitude = parse_amount_cents(row.get(*amount_col)?)?.abs();
            let label = row.get(*type_col)?.trim();
            let matches = |list: &[String]| list.iter().any(|l| l.eq_ignore_ascii_case(label));
            let direction = if matches(debit_labels) {
                Direction::Debit
            } else if matches(credit_labels) {
                Direction::Credit
            } else {
                return None;
            };
            Some((magnitude, direction))
        }
    }
}

/// Parse one CSV row into a Transaction using `inst`.
/// Returns None when the date, vendor, or amount cannot be resolved.
pub fn parse_row(inst: &Institution, row: &[String]) -> Option<Transaction> {
    let date = parse_date(row.get(inst.date_col)?)?;
    let vendor = row.get(inst.vendor_col)?.trim().to_string();
    let (amount_cents, direction) = resolve_amount(&inst.amount, row)?;
    let category = match inst.category_col {
        Some(col) => row
            .get(col)
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
        None => String::new(),
    };
    Some(Transaction {
        date,
        vendor,
        category,
        amount_cents,
        direction,
    })
}

// ── Heuristic suggestion ──────────────────────────────────────────────────────

/// Find the first header index whose lowercased text contains any keyword.
fn find_col(headers: &[String], keywords: &[&str]) -> Option<usize> {
    headers.iter().position(|h| {
        let lh = h.trim().to_lowercase();
        keywords.iter().any(|k| lh.contains(k))
    })
}

/// Produce an initial mapping guess from header names.
/// Defaults to the SingleSigned scheme (most common for card statements);
/// the modal lets the user correct any field.
pub fn suggest_mapping(headers: &[String]) -> SuggestedMapping {
    let date_col = find_col(headers, &["transaction date", "trans date"])
        .or_else(|| find_col(headers, &["date"]))
        .unwrap_or(0);

    let vendor_col = find_col(
        headers,
        &["description", "payee", "merchant", "vendor", "name"],
    )
    .or_else(|| find_col(headers, &["memo"]))
    .unwrap_or(0);

    let amount_col = find_col(headers, &["amount", "amt"]).unwrap_or(0);
    let type_col = find_col(headers, &["type"]);
    let category_col = find_col(headers, &["category"]);

    // Hide every column not used as date, vendor, amount, or category by default.
    let ignore_cols = (0..headers.len())
        .filter(|i| {
            *i != date_col && *i != vendor_col && *i != amount_col && Some(*i) != category_col
        })
        .collect();

    SuggestedMapping {
        date_col,
        vendor_col,
        amount_col,
        type_col,
        category_col,
        scheme: hho_types::AmountSchemeTag::SingleSigned,
        debit_is_negative: true,
        ignore_cols,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|s| s.to_string()).collect()
    }

    // fingerprint ───────────────────────────────────────────────────────────────

    #[test]
    fn fingerprint_normalizes_case_and_whitespace() {
        let h = row(&["Transaction Date", " Post Date ", "AMOUNT"]);
        assert_eq!(fingerprint(&h), "transaction date,post date,amount");
    }

    #[test]
    fn find_institution_matches_by_fingerprint() {
        let inst = chase();
        let list = vec![inst.clone()];
        assert_eq!(find_institution(&inst.fingerprint, &list), Some(&list[0]));
        assert_eq!(find_institution("nope", &list), None);
    }

    // parse_date ─────────────────────────────────────────────────────────────────

    #[test]
    fn date_parses_common_formats() {
        assert_eq!(parse_date("05/18/2026").as_deref(), Some("2026-05-18"));
        assert_eq!(parse_date("2026-05-18").as_deref(), Some("2026-05-18"));
        assert_eq!(parse_date("18-May-2026").as_deref(), Some("2026-05-18"));
        assert_eq!(parse_date("05/18/26").as_deref(), Some("2026-05-18"));
    }

    #[test]
    fn date_rejects_unknown() {
        assert_eq!(parse_date("not a date"), None);
        assert_eq!(parse_date(""), None);
    }

    // ── Real Chase fixture ──────────────────────────────────────────────────────
    // Header: Transaction Date,Post Date,Description,Category,Type,Amount,Memo

    fn chase() -> Institution {
        Institution {
            name: "Chase Sapphire".into(),
            fingerprint: "transaction date,post date,description,category,type,amount,memo".into(),
            date_col: 0,
            vendor_col: 2,
            category_col: Some(3),
            ignore_cols: vec![1, 4, 6],
            amount: AmountScheme::SingleSigned {
                amount_col: 5,
                debit_is_negative: true,
            },
        }
    }

    #[test]
    fn chase_sale_is_debit() {
        let r = row(&[
            "05/18/2026",
            "05/19/2026",
            "BUDGET RENT A CAR",
            "Travel",
            "Sale",
            "-286.97",
            "",
        ]);
        let t = parse_row(&chase(), &r).unwrap();
        assert_eq!(t.date, "2026-05-18");
        assert_eq!(t.vendor, "BUDGET RENT A CAR");
        assert_eq!(t.category, "Travel");
        assert_eq!(t.amount_cents, 28697);
        assert_eq!(t.direction, Direction::Debit);
    }

    #[test]
    fn chase_payment_is_credit() {
        let r = row(&[
            "04/22/2026",
            "04/22/2026",
            "AUTOMATIC PAYMENT - THANK",
            "",
            "Payment",
            "3367.17",
            "",
        ]);
        let t = parse_row(&chase(), &r).unwrap();
        assert_eq!(t.amount_cents, 336717);
        assert_eq!(t.direction, Direction::Credit);
        assert_eq!(t.category, "");
    }

    // ── Type-column scheme ──────────────────────────────────────────────────────

    #[test]
    fn type_column_resolves_direction_by_label() {
        let scheme = AmountScheme::TypeColumn {
            amount_col: 2,
            type_col: 3,
            debit_labels: vec!["DEBIT".into(), "DR".into()],
            credit_labels: vec!["CREDIT".into(), "CR".into()],
        };
        let debit = row(&["x", "y", "50.00", "debit"]); // case-insensitive
        let credit = row(&["x", "y", "50.00", "CR"]);
        assert_eq!(
            resolve_amount(&scheme, &debit),
            Some((5000, Direction::Debit))
        );
        assert_eq!(
            resolve_amount(&scheme, &credit),
            Some((5000, Direction::Credit))
        );
    }

    #[test]
    fn type_column_unmatched_label_returns_none() {
        let scheme = AmountScheme::TypeColumn {
            amount_col: 0,
            type_col: 1,
            debit_labels: vec!["DEBIT".into()],
            credit_labels: vec!["CREDIT".into()],
        };
        assert_eq!(resolve_amount(&scheme, &row(&["1.00", "MYSTERY"])), None);
    }

    // ── Out-of-range safety ─────────────────────────────────────────────────────

    #[test]
    fn parse_row_handles_short_row() {
        let short = row(&["05/18/2026"]); // missing vendor/amount columns
        assert_eq!(parse_row(&chase(), &short), None);
    }

    // ── suggest_mapping ─────────────────────────────────────────────────────────

    #[test]
    fn suggest_mapping_picks_chase_columns() {
        let h = row(&[
            "Transaction Date",
            "Post Date",
            "Description",
            "Category",
            "Type",
            "Amount",
            "Memo",
        ]);
        let s = suggest_mapping(&h);
        assert_eq!(s.date_col, 0);
        assert_eq!(s.vendor_col, 2);
        assert_eq!(s.amount_col, 5);
        assert_eq!(s.type_col, Some(4));
        assert_eq!(s.category_col, Some(3));
        assert_eq!(s.ignore_cols, vec![1, 4, 6]);
    }

    // ── Serialization round-trips (the cross-crate / persistence contract) ──────

    #[test]
    fn institution_roundtrips_through_toml() {
        let original = chase();
        let toml_str = toml::to_string_pretty(&original).unwrap();
        // Confirms the flattened enum produces a flat, readable table.
        assert!(toml_str.contains("amount_scheme = \"single_signed\""));
        assert!(toml_str.contains("amount_col = 5"));
        assert!(toml_str.contains("debit_is_negative = true"));
        let recovered: Institution = toml::from_str(&toml_str).unwrap();
        assert_eq!(recovered, original);
    }

    #[test]
    fn institution_roundtrips_through_json() {
        // Mirrors the frontend → save_mapping path (serde_json shape).
        let original = chase();
        let json = serde_json::to_value(&original).unwrap();
        assert_eq!(json["amount_scheme"], "single_signed");
        let recovered: Institution = serde_json::from_value(json).unwrap();
        assert_eq!(recovered, original);
    }

    #[test]
    fn type_column_institution_roundtrips_through_toml() {
        let inst = Institution {
            name: "CU".into(),
            fingerprint: "date,description,amount,transaction type".into(),
            date_col: 0,
            vendor_col: 1,
            category_col: None,
            ignore_cols: vec![],
            amount: AmountScheme::TypeColumn {
                amount_col: 2,
                type_col: 3,
                debit_labels: vec!["DEBIT".into()],
                credit_labels: vec!["CREDIT".into()],
            },
        };
        let toml_str = toml::to_string_pretty(&inst).unwrap();
        let recovered: Institution = toml::from_str(&toml_str).unwrap();
        assert_eq!(recovered, inst);
    }
}
