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
| F  | Pane headers          | **Yes** — Joint / Uncategorized / Mine / Ignored  |
