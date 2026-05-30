# Application Plan

## Stack

- **Frontend**: Leptos 0.7 (CSR / WASM), compiled with Trunk
- **Shell**: Tauri 2.x
- **Styling**: CSS (replacing existing `styles.css`), no external CSS framework

---

## Layout

```
┌──────────────────────────────────────────────────────────┐
│  Menu bar: [File ▾]  →  File > Quit                      │
├──────────────┬─────────────────┬────────────────────────┤
│  Left pane   │  Middle pane    │  Right pane            │
│              │  ─ thing 1 (*)  │                        │
│              │  ─ thing 2      │                        │
│              │  ─ thing 3      │                        │
│              │                 │                        │
├──────────────┴─────────────────┴────────────────────────┤
│  Bottom pane (full width)                                │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

- Background: `#1e1e1e` (very dark gray)
- Pane dividers: light-orange (`#f5a623` at ~40% opacity, 1–2 px)
- Row separators: light gray (`#555` at ~30% opacity, 1 px)
- Selected row: highlighted with a muted accent (e.g. `#3a5a8a` or similar)
- Active pane: subtle border/glow to indicate which pane has keyboard focus

---

## Component Architecture

```
App
├── MenuBar          ← file menu with Quit
├── MainArea
│   ├── TopSection
│   │   ├── Pane (left)
│   │   ├── Pane (middle) ← initial data: thing 1*, thing 2, thing 3
│   │   └── Pane (right)
│   └── BottomPane
└── DebugLog         ← live debug output panel (TBD: see open questions)
```

---

## State Model

```rust
/// Which of the four panes is currently "active" (has keyboard focus)
enum ActivePane { Left, Middle, Right, Bottom }

/// An item that lives in a pane
struct Item { id: u32, label: String }

/// Top-level reactive state
struct AppState {
    active_pane: ActivePane,         // starts: Middle
    left_items:   Vec<Item>,         // starts: []
    middle_items: Vec<Item>,         // starts: [thing1, thing2, thing3]
    right_items:  Vec<Item>,         // starts: []
    bottom_items: Vec<Item>,         // starts: []
    left_sel:     Option<usize>,     // selected row index in left pane
    middle_sel:   Option<usize>,     // starts: Some(0) → "thing 1"
    right_sel:    Option<usize>,
    bottom_sel:   Option<usize>,
    debug_log:    Vec<String>,       // append-only log of events
}
```

---

## Keyboard Event Handling

All keyboard events are captured on the root `<div>` (with `tabindex="0"` so it receives focus, or on `document` via `window_event_listener`).

| Key             | Action |
|----------------|--------|
| `ArrowUp`       | Move selection up one row in active pane (clamp at top) |
| `ArrowDown`     | Move selection down one row in active pane (clamp at bottom) |
| `ArrowLeft`     | Switch active pane: Right→Middle→Left (no wrap) |
| `ArrowRight`    | Switch active pane: Left→Middle→Right (no wrap) |
| `Shift+ArrowLeft`  | Move selected item from active top pane to pane on the left; no-op if already leftmost or if active pane is bottom |
| `Shift+ArrowRight` | Move selected item from active top pane to pane on the right; no-op if already rightmost or if active pane is bottom |
| `Shift+ArrowDown`  | Move selected item from active top pane to bottom pane |
| `Shift+ArrowUp`    | Move selected item from bottom pane to middle pane; no-op if active pane is not bottom |

**Left/Right arrow (pane switching)**: The bottom pane is not reachable by left/right arrow — it is only activated by clicking it or by an item landing in it via Shift+Down.

**Arrow navigation in bottom pane**: Up/Down arrows work normally within the bottom pane when it is active.

---

## Mouse / Click Handling

- Clicking anywhere inside a pane makes that pane the active pane.
- If the clicked pane has items, the clicked row becomes selected; if no row was clicked (click in empty space), keep existing selection for that pane.
- Clicking a row in an inactive pane: (1) activates the pane, (2) selects that row.

---

## Item Movement Semantics

1. The item at the currently selected row index is moved.
2. It is **appended** to the end of the destination pane's item list.
3. After removal from the source pane, the selection in the source pane moves to `min(old_index, new_len - 1)`, or `None` if the source pane is now empty.
4. The active pane **stays the same** (focus does not follow the item).
5. If the destination pane was previously empty, its selection becomes `Some(0)` — but focus stays on the source pane.

---

## Menu Bar

**Decision: Native OS menu bar** via Tauri's `Menu` API (Rust side).

- Defined in `src-tauri/src/lib.rs` using `tauri::menu::{Menu, Submenu, MenuItem}`.
- "File > Quit" calls `AppHandle::exit(0)`.
- No HTML menu component needed.

---

## Debug Output

**Decision: In-app scrollable panel**, rendered below the bottom pane.

- A `<div class="debug-log">` with `overflow-y: scroll`, fixed height (e.g. 150 px).
- Entries prepended (newest at top) so the latest event is always visible without scrolling.
- Every event logged with: event type, key/button, active pane before action, action taken (or "no-op: reason"), resulting state.
- Also echoed to `web_sys::console::log_1` for convenience in dev tools.

---

## File Structure (target)

```
src/
  main.rs          ← mount App
  app.rs           ← top-level App component + global key handler
  state.rs         ← AppState, Item, ActivePane
  components/
    menu_bar.rs    ← MenuBar component
    pane.rs        ← generic Pane component
    bottom_pane.rs ← BottomPane component
    debug_log.rs   ← DebugLog component (if in-app panel)

src-tauri/src/
  lib.rs           ← Tauri commands (quit, etc.)
  main.rs

styles.css         ← complete rewrite for new layout
```

---

## Implementation Phases

1. **Layout skeleton** — CSS grid structure, dark background, pane dividers, row separators
2. **State + data** — `AppState` reactive signals, seed data (thing 1/2/3)
3. **Keyboard handler** — global event listener, all key bindings, debug logging
4. **Mouse handler** — click-to-activate-pane, click-to-select-row
5. **Item movement** — shift+arrow mechanics
6. **Menu bar** — native or custom (pending answer to Question A)
7. **Debug output** — panel or console (pending answer to Question B)
8. **Polish** — visual active-pane indicator, scroll-into-view for selected rows

---

## Resolved Decisions

| \# | Question              | Decision                                          |
|----|-----------------------|---------------------------------------------------|
| A  | Menu bar type         | **Native OS menu** via Tauri Menu API             |
| B  | Debug output location | **In-app scrollable panel** (+ console.log)       |
| C  | Post-move focus       | **Stay in source pane**                           |
| D  | Bottom pane layout    | **Single flat list**                              |
| E  | Active pane indicator | **Slightly lighter background** on active pane    |
| F  | Pane headers          | **Yes** — Joint / Unassigned / Mine / Ignored  |

---

## Feature: CSV Transaction Mapping

Banks and credit-card CSVs are per-institution but share four extractable
fields: **Date**, **Vendor**, **Amount**, **Direction** (Debit/Credit). HHO
learns a per-institution column mapping, keyed off the header row, and persists
it in `hho_user_config.toml`.

### Feature Decisions

| \# | Question                  | Decision                                                  |
|----|---------------------------|-----------------------------------------------------------|
| 1  | Debit/Credit schemes      | **Single-signed** + **Type-column** only (Split deferred) |
| 2  | Date/Amount handling      | **Structured** — integer cents + canonical ISO date       |
| 3  | v1 mapping UI             | **Modal on unknown header only** (no manager screen)      |
| 4  | Mapping key               | Normalized header-row **fingerprint** (exact match)       |

### Open / Apply Flow

```
read header row
  → fingerprint = headers.map(trim+lowercase).join(",")
  → look up fingerprint among saved institutions
      ├── found    → parse each row → Vec<Transaction> → Unassigned pane
      └── not found → emit NeedsMapping → frontend shows modal
                       → user maps → save_mapping → parse → Unassigned pane
```

### Data Model (backend `src-tauri/src/mapping.rs`)

```rust
enum Direction { Debit, Credit }

// v1 supports two schemes; Split { debit_col, credit_col } is a future variant.
enum AmountScheme {
    SingleSigned { amount_col: usize, debit_is_negative: bool },
    TypeColumn   { amount_col: usize, type_col: usize,
                   debit_labels: Vec<String>, credit_labels: Vec<String> },
}

struct Institution {
    name:        String,
    fingerprint: String,       // normalized header key
    date_col:    usize,
    vendor_col:  usize,
    ignore_cols: Vec<usize>,   // hidden from the UI ("hidden columns")
    amount:      AmountScheme,
}

struct Transaction {
    date:         String,  // canonical "YYYY-MM-DD" (sorts chronologically as text)
    vendor:       String,
    amount_cents: i64,     // magnitude, always >= 0  (integer cents, never f64)
    direction:    Direction,
}
```

**BKM — money as integer cents.** Tolerant `parse_amount_cents` handles `$`,
thousands commas, `(5.40)` parentheses-negatives, and trailing `CR`/`DR`.

**BKM — dates to canonical ISO.** `parse_date` tries an ordered list of common
formats (`MM/DD/YYYY`, `M/D/YYYY`, `YYYY-MM-DD`, `DD-Mon-YYYY`, …) and emits
`YYYY-MM-DD`; lexical order then equals chronological order.

### Config File Additions (`~/hho_user_config.toml`)

```toml
[[institution]]
name = "Chase Sapphire"
fingerprint = "trans date,description,category,type,amount"
date_col = 0
vendor_col = 1
ignore_cols = [2, 3]
amount_scheme = "single_signed"
amount_col = 4
debit_is_negative = true

[[institution]]
name = "Credit Union"
fingerprint = "date,description,amount,transaction type"
date_col = 0
vendor_col = 1
ignore_cols = []
amount_scheme = "type_column"
amount_col = 2
type_col = 3
debit_labels = ["DEBIT", "DR", "Sale"]
credit_labels = ["CREDIT", "CR", "Payment"]
```

### Column-Chooser Modal (shown only for unknown headers)

```
┌─ Map Columns — New Institution ─────────────────────────────┐
│ Institution name: [ Chase Sapphire________ ]                │
│ Preview (first 3 rows):                                     │
│ ┌──────────┬────────────┬──────────┬────────┬──────────┐   │
│ │Trans Date│Description │ Category │ Type   │ Amount   │   │
│ │01/15/2026│STARBUCKS   │Dining    │ Sale   │ -5.40    │   │
│ └──────────┴────────────┴──────────┴────────┴──────────┘   │
│ Transaction Date → [ Trans Date ▾ ]                         │
│ Vendor Name      → [ Description ▾ ]                        │
│ Amount/Direction: (•) Single signed  ( ) Type column        │
│   Amount column → [ Amount ▾ ]   Debit is (•)neg ( )pos     │
│ Hidden columns: [ ]Date [ ]Desc [✓]Category [✓]Type [ ]Amt  │
│                       [ Cancel ]   [ Save & Apply ]         │
└─────────────────────────────────────────────────────────────┘
```

Backend computes **heuristic pre-selections** from header names
(`date`/`posted` → Date; `description`/`payee`/`merchant` → Vendor;
`amount`/`amt` → Amount); the user usually just confirms.

### IPC Changes

- `pick_csv` / `open_csv` return `Mapped { name, transactions }`
  **or** `NeedsMapping { fingerprint, headers, sample_rows, suggested }`.
- new `save_mapping(institution, pending_path) -> Vec<Transaction>`
  (persists institution, then parses the pending file).

### Frontend Changes

- `Item` carries a `Transaction`; renders a one-line label
  (`2026-01-15 │ Starbucks │ −$5.40`).
- new `MappingModal` component driven by
  `pending_mapping: RwSignal<Option<NeedsMapping>>` in `state.rs`.
- `populate_middle_pane` consumes structured transactions.

### Terminology Note

**Hidden columns** (this feature) ≠ the **Ignored pane** (existing bottom
bucket). Modal label uses "Hidden columns" to avoid the collision.

### Testing Plan (native unit tests in `mapping.rs`)

`fingerprint` normalization · `find_institution` hit/miss · `parse_amount_cents`
across `$1,234.56` / `(5.40)` / `5.40 CR` / `-5.40` · `parse_date` across each
supported format · `resolve_direction` for both schemes · `parse_row`
end-to-end per scheme · `suggest_mapping` heuristics · TOML round-trip of each
`AmountScheme`.

### Phasing

1. **Backend core** — fingerprint, config schema, 2-scheme parser + date/amount
   parsers, auto-apply known institutions, return `NeedsMapping`. Full tests.
2. **Modal UI** — column chooser, heuristic suggestions, `save_mapping`.
