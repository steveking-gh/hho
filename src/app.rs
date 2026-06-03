// Root application component.
// Owns AppState, provides it via context, and registers all global event
// listeners: keyboard (nav / zoom), mouse (drag-resize), window resize.

use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::{
    create_transaction_modal::CreateTransactionModal,
    debug_log::DebugLog,
    header::Header,
    mapping_modal::MappingModal,
    month_modal::MonthModal,
    pane::Pane,
    print_view::PrintView,
    resize_handle::{ResizeDir, ResizeHandle},
    rule_editor_modal::RuleEditorModal,
    rules_modal::RulesModal,
    split_transaction_modal::SplitTransactionModal,
    transaction_editor_modal::TransactionEditorModal,
    nickname_editor_modal::NicknameEditorModal,
    nickname_rules_modal::NicknameRulesModal,
};
use crate::dto::{OpenResult, PendingMapping};
use crate::ipc;
use crate::logic::{nav_down, nav_up, pane_left, pane_right, ActivePane, Item};
use crate::state::{AppState, DragState, DragTarget, PANE_MIN_H, PANE_MIN_W};

// ── Scale constants ───────────────────────────────────────────────────────────

const SCALE_MIN: f32 = 7.0;
const SCALE_MAX: f32 = 20.0;
const SCALE_STEP: f32 = 1.0;
const SCALE_DEFAULT: f32 = 10.0;

// IPC argument/result types live in the shared hho-types crate and are used
// through the typed wrappers in `crate::ipc`.

// ── DOM helpers ───────────────────────────────────────────────────────────────

/// Set `font-size` on `<html>` — all rem-based sizes scale proportionally.
fn apply_font_scale(px: f32) {
    let _ = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|html| {
            let _ = html
                .style()
                .set_property("font-size", &format!("{:.1}px", px));
        });
}

/// Set (or clear) a global drag cursor on `<html>` so the cursor stays
/// consistent even when the pointer travels over non-handle elements.
fn set_drag_cursor(cursor: &str) {
    let _ = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|html| {
            let style = html.style();
            if cursor.is_empty() {
                let _ = style.remove_property("cursor");
                let _ = style.remove_property("user-select");
            } else {
                let _ = style.set_property("cursor", cursor);
                let _ = style.set_property("user-select", "none");
            }
        });
}

// ── Open-result dispatch ──────────────────────────────────────────────────────

/// Apply the backend's open result: render transactions, open the mapping
/// modal, or log a cancellation.
pub(crate) fn handle_open_result(state: AppState, result: OpenResult) {
    match result {
        OpenResult::Mapped {
            institution,
            transactions,
        } => {
            state.populate_transactions(&institution, transactions);
        }
        OpenResult::NeedsMapping {
            fingerprint,
            headers,
            sample_rows,
            pending_path,
            suggested,
        } => {
            state.log(format!(
                "[File] unknown institution (fingerprint=\"{fingerprint}\") → opening mapping modal"
            ));
            state.pending_mapping.set(Some(PendingMapping {
                fingerprint,
                headers,
                sample_rows,
                pending_path,
                suggested,
            }));
        }
        OpenResult::Cancelled => state.log("[File] open cancelled".to_string()),
    }
}

/// Searches for an existing auto-assign rule matching the vendor and description.
fn find_matching_rule(state: AppState, vendor: &str, description: &str) -> Option<(usize, hho_types::AutoAssignRule)> {
    let rules = state.auto_assign_rules.get_untracked();
    for (idx, r) in rules.iter().enumerate() {
        if let Some(compiled) = crate::logic::CompiledRule::new(r) {
            if compiled.matches(vendor, description) {
                return Some((idx, r.clone()));
            }
        }
    }
    None
}

/// Properties parsed for initializing the auto-assign rule editor modal.
#[derive(Clone, Debug, PartialEq)]
pub struct AssignModalProps {
    pub preview_vendor: String,
    pub preview_description: String,
    pub initial_vendor_regex: String,
    pub initial_description_regex: String,
    pub initial_pane: hho_types::RulePane,
    pub initial_category_override: String,
}

/// Builds the initialization properties for the auto-assign rule editor modal.
pub fn build_assign_modal_props(state: AppState, item: &Item) -> AssignModalProps {
    let vendor = item.txn.vendor.clone();
    let description = item.txn.description.clone();
    let escaped_vendor = crate::logic::escape_regex(&vendor);
    let matched_info = find_matching_rule(state, &vendor, &description);
    let initial_vendor_regex = matched_info
        .as_ref()
        .map(|(_, r)| r.vendor_pattern().unwrap_or("").to_string())
        .unwrap_or_else(|| escaped_vendor.clone());
    let initial_description_regex = matched_info
        .as_ref()
        .map(|(_, r)| r.description_pattern().unwrap_or("").to_string())
        .unwrap_or_default();
    let initial_pane = matched_info
        .as_ref()
        .map(|(_, r)| r.pane)
        .unwrap_or(hho_types::RulePane::Joint);
    let initial_category_override = matched_info
        .as_ref()
        .map(|(_, r)| r.category_override.clone().unwrap_or_default())
        .unwrap_or_default();

    AssignModalProps {
        preview_vendor: vendor,
        preview_description: description,
        initial_vendor_regex,
        initial_description_regex,
        initial_pane,
        initial_category_override,
    }
}

/// Saves an auto-assign rule to the persistent configuration.
pub async fn save_rule(state: AppState, rule: hho_types::AutoAssignRule, vendor: &str, description: &str) {
    let matched_info = find_matching_rule(state, vendor, description);
    let mut rules = state.auto_assign_rules.get_untracked();
    if let Some((idx, _)) = matched_info {
        if idx < rules.len() {
            rules[idx] = rule.clone();
        }
    } else {
        rules.push(rule.clone());
    }

    state.log(format!(
        "[AutoAssign] saving {} rules: pattern=\"{}\" target={}",
        rules.len(),
        rule.display_pattern(),
        rule.pane
    ));

    if let Err(e) = crate::ipc::save_auto_assign_rules(state, rules.clone()).await {
        state.log(format!("[AutoAssign] failed to save rules list: {e}"));
    } else {
        state.auto_assign_rules.set(rules);
        state.apply_month_filter();
    }
    state.assign_modal_item.set(None);
}

/// Finds an existing nickname rule matching the regex pattern.
fn find_nickname_rule(state: AppState, regex: &str) -> Option<usize> {
    let rules = state.nickname_rules.get_untracked();
    rules.iter().position(|r| r.regex == regex)
}

/// Saves a nickname rule to the persistent configuration.
pub async fn save_nickname_rule(state: AppState, rule: hho_types::NicknameRule) {
    let mut rules = state.nickname_rules.get_untracked();
    if let Some(idx) = find_nickname_rule(state, &rule.regex) {
        rules[idx] = rule.clone();
    } else {
        rules.push(rule.clone());
    }

    state.log(format!(
        "[Nicknames] saving {} nickname rules: regex=\"{}\" nickname=\"{}\"",
        rules.len(),
        rule.regex,
        rule.nickname
    ));

    if let Err(e) = crate::ipc::save_nickname_rules(state, rules.clone()).await {
        state.log(format!("[Nicknames] failed to save nickname rules list: {e}"));
    } else {
        state.nickname_rules.set(rules);
        state.apply_month_filter();
    }
    state.nickname_modal_item.set(None);
}

// ── Layout restore ────────────────────────────────────────────────────────────

thread_local! {
    static LAST_SAVED_LAYOUT: std::cell::Cell<(f32, f32, f32, f32)> = const { std::cell::Cell::new((0.0, 0.0, 0.0, 0.0)) };
}

fn load_layout_from_config(state: AppState) {
    spawn_local(async move {
        if let Ok(layout) = ipc::get_layout(state).await {
            state.left_width.set(layout.left_width);
            state.right_width.set(layout.right_width);
            state.bottom_h.set(layout.bottom_h);
            state.debug_h.set(layout.debug_h);
            LAST_SAVED_LAYOUT.with(|cell| {
                cell.set((layout.left_width, layout.right_width, layout.bottom_h, layout.debug_h));
            });
            state.log(format!(
                "[Init] layout restored: left={:.0}px right={:.0}px bottom={:.0}px debug={:.0}px",
                layout.left_width, layout.right_width, layout.bottom_h, layout.debug_h,
            ));
        }
    });
}

fn load_rules_from_config(state: AppState) {
    spawn_local(async move {
        if let Ok(rules) = ipc::get_auto_assign_rules(state).await {
            state.auto_assign_rules.set(rules);
            state.log(format!(
                "[Init] auto-assign rules restored: count={}",
                state.auto_assign_rules.get_untracked().len()
            ));
            if !state.raw_transactions.get_untracked().is_empty() {
                state.apply_month_filter();
            }
        }
    });
}

fn load_nicknames_from_config(state: AppState) {
    spawn_local(async move {
        if let Ok(rules) = ipc::get_nickname_rules(state).await {
            state.nickname_rules.set(rules);
            state.log(format!(
                "[Init] nickname rules restored: count={}",
                state.nickname_rules.get_untracked().len()
            ));
            if !state.raw_transactions.get_untracked().is_empty() {
                state.apply_month_filter();
            }
        }
    });
}

// ── App component ─────────────────────────────────────────────────────────────

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    provide_context(state);

    // Determines and sets the initial month and year from the current clock.
    let now = js_sys::Date::new_0();
    let current_y = now.get_full_year() as i32;
    let current_m = now.get_month() as i32 + 1;
    let (prev_y, prev_m) = crate::logic::get_previous_month_year(current_y, current_m);
    state.selected_year.set(prev_y);
    state.selected_month.set(prev_m);

    state.refresh_recent_files();
    load_layout_from_config(state);
    load_rules_from_config(state);
    load_nicknames_from_config(state);

    // ── Global mouse-move handler (drag-resize) ───────────────────────────────
    let move_handle = window_event_listener(ev::mousemove, move |ev| {
        let Some(drag) = state.drag.get_untracked() else {
            return;
        };

        let cx = ev.client_x() as f32;
        let cy = ev.client_y() as f32;
        let dx = cx - drag.last_x;
        let dy = cy - drag.last_y;

        state.drag.set(Some(DragState {
            last_x: cx,
            last_y: cy,
            ..drag
        }));

        match drag.target {
            DragTarget::LeftHandle => {
                let new = (state.left_width.get_untracked() + dx).max(PANE_MIN_W);
                state.left_width.set(new);
            }
            DragTarget::RightHandle => {
                let new = (state.right_width.get_untracked() - dx).max(PANE_MIN_W);
                state.right_width.set(new);
            }
            DragTarget::TopHandle => {
                let new = (state.bottom_h.get_untracked() - dy).max(PANE_MIN_H);
                state.bottom_h.set(new);
            }
            DragTarget::BottomHandle => {
                let new_bot = (state.bottom_h.get_untracked() + dy).max(PANE_MIN_H);
                let new_dbg = (state.debug_h.get_untracked() - dy).max(PANE_MIN_H);
                state.bottom_h.set(new_bot);
                state.debug_h.set(new_dbg);
            }
        }
    });

    // ── Global mouse-up handler (end drag, save layout) ───────────────────────
    let up_handle = window_event_listener(ev::mouseup, move |_ev| {
        let Some(drag) = state.drag.get_untracked() else {
            return;
        };
        state.drag.set(None);
        set_drag_cursor("");

        let left_w = state.left_width.get_untracked();
        let right_w = state.right_width.get_untracked();
        let bot_h = state.bottom_h.get_untracked();
        let dbg_h = state.debug_h.get_untracked();

        let changed = LAST_SAVED_LAYOUT.with(|cell| {
            let last = cell.get();
            if last != (left_w, right_w, bot_h, dbg_h) {
                cell.set((left_w, right_w, bot_h, dbg_h));
                true
            } else {
                false
            }
        });

        if changed {
            state.log(format!(
                "[Drag] end {:?} → left={left_w:.0}px right={right_w:.0}px bottom={bot_h:.0}px debug={dbg_h:.0}px  (saving…)",
                drag.target,
            ));

            spawn_local(async move {
                ipc::save_layout(state, left_w, right_w, bot_h, dbg_h).await;
            });
        }
    });

    // ── Global key-down handler ───────────────────────────────────────────────
    let key_handle = window_event_listener(ev::keydown, move |ev| {
        let key = ev.key();
        let shift = ev.shift_key();
        let ctrl = ev.ctrl_key();

        // ── Ctrl+zoom ─────────────────────────────────────────────────────────
        if ctrl && matches!(key.as_str(), "=" | "+" | "-" | "0") {
            ev.prevent_default();
            let current = state.font_scale.get_untracked();
            let (new_scale, action) = match key.as_str() {
                "=" | "+" => ((current + SCALE_STEP).min(SCALE_MAX), "zoom in"),
                "-" => ((current - SCALE_STEP).max(SCALE_MIN), "zoom out"),
                _ => (SCALE_DEFAULT, "zoom reset"),
            };
            state.font_scale.set(new_scale);
            apply_font_scale(new_scale);
            state.log(format!(
                "[KeyDown] Ctrl+{key}  →  {action} | scale={new_scale:.1}px/rem"
            ));
            return;
        }

        // ── Keyboard shortcuts ────────────────────────────────────────────────
        if ctrl && (key.eq_ignore_ascii_case("o") || key.eq_ignore_ascii_case("q")) {
            ev.prevent_default();
            if key.eq_ignore_ascii_case("o") {
                if state.is_loading_file.get_untracked() || state.any_modal_open() {
                    return;
                }
                state.is_loading_file.set(true);
                state.log("[Shortcut] Ctrl+O → invoking pick_csv".to_string());
                spawn_local(async move {
                    match ipc::pick_csv(state).await {
                        Ok(r) => handle_open_result(state, r),
                        Err(e) => state.log(format!("[File] pick_csv failed: {e}")),
                    }
                    state.is_loading_file.set(false);
                });
            } else {
                state.log("[Shortcut] Ctrl+Q → exiting app".to_string());
                spawn_local(async move {
                    ipc::exit_app(state).await;
                });
            }
            return;
        }

        // Suppress navigation while any modal is open.
        if state.any_modal_open() {
            return;
        }

        // ── Enter key handling ────────────────────────────────────────────────
        if key == "Enter" {
            let pane = state.active_pane.get_untracked();
            if pane == ActivePane::Middle {
                let items = state.middle_items.get_untracked();
                let sel = state.middle_sel.get_untracked();
                if let Some(idx) = sel {
                    if idx < items.len() {
                        ev.prevent_default();
                        let selected_item = items[idx].clone();
                        if shift {
                            state.editing_transaction_item.set(Some(selected_item));
                            state.log(format!(
                                "[KeyDown] Shift+Enter  →  opening edit transaction modal for row {idx}"
                            ));
                        } else {
                            state.assign_modal_item.set(Some(selected_item));
                            state.log(format!(
                                "[KeyDown] Enter  →  opening auto-assign modal for row {idx}"
                            ));
                        }
                        return;
                    }
                }
            }
        }

        // Trigger split transaction modal on 'S' key press if Unassigned pane holds active selection.
        if key.eq_ignore_ascii_case("s") {
            let pane = state.active_pane.get_untracked();
            if pane == ActivePane::Middle {
                let items = state.middle_items.get_untracked();
                let sel = state.middle_sel.get_untracked();
                if let Some(idx) = sel {
                    if idx < items.len() {
                        ev.prevent_default();
                        let selected_item = items[idx].clone();
                        state.split_transaction_item.set(Some(selected_item));
                        state.log(format!(
                            "[KeyDown] S  →  opening split transaction modal for row {idx}"
                        ));
                        return;
                    }
                }
            }
        }

        // Trigger nickname modal on 'N' key press if the active pane holds an active selection.
        if key.eq_ignore_ascii_case("n") {
            let pane = state.active_pane.get_untracked();
            let items = state.items_for(pane).get_untracked();
            let sel = state.sel_for(pane).get_untracked();
            if let Some(idx) = sel {
                if idx < items.len() {
                    ev.prevent_default();
                    let selected_item = items[idx].clone();
                    state.nickname_modal_item.set(Some(selected_item));
                    state.log(format!(
                        "[KeyDown] N  →  opening nickname modal for row {idx} in {pane}"
                    ));
                    return;
                }
            }
        }

        // ── Arrow-key guard ───────────────────────────────────────────────────
        if !matches!(
            key.as_str(),
            "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
        ) {
            return;
        }

        let pane = state.active_pane.get_untracked();
        ev.prevent_default();

        let prefix = format!(
            "[KeyDown] {shift_str}{key:<14} active={pane:<14} sel={sel:?}",
            shift_str = if shift { "Shift+" } else { "" },
            key = key,
            pane = pane.to_string(),
            sel = state.sel_for(pane).get_untracked(),
        );

        let detail: String = match (shift, key.as_str()) {
            (false, "ArrowUp") => {
                let items = state.items_for(pane).get_untracked();
                let sel = state.sel_for(pane).get_untracked();
                let new_sel = nav_up(&items, sel);
                state.sel_for(pane).set(new_sel);
                format!("nav up → sel={:?}", new_sel)
            }
            (false, "ArrowDown") => {
                let items = state.items_for(pane).get_untracked();
                let sel = state.sel_for(pane).get_untracked();
                let new_sel = nav_down(&items, sel);
                state.sel_for(pane).set(new_sel);
                format!("nav down → sel={:?}", new_sel)
            }
            (false, "ArrowLeft") => {
                let next = pane_left(pane);
                state.active_pane.set(next);
                if next == pane {
                    "switch left → no-op".into()
                } else {
                    format!("switch left → active=\"{}\"", next)
                }
            }
            (false, "ArrowRight") => {
                let next = pane_right(pane);
                state.active_pane.set(next);
                if next == pane {
                    "switch right → no-op".into()
                } else {
                    format!("switch right → active=\"{}\"", next)
                }
            }
            (true, "ArrowLeft") => match pane {
                ActivePane::Middle => state.transfer(ActivePane::Middle, ActivePane::Left),
                ActivePane::Right => state.transfer(ActivePane::Right, ActivePane::Middle),
                ActivePane::Left => "no-op: no pane left of Joint".into(),
                ActivePane::Bottom => "no-op: Ignored has no left neighbor".into(),
            },
            (true, "ArrowRight") => match pane {
                ActivePane::Left => state.transfer(ActivePane::Left, ActivePane::Middle),
                ActivePane::Middle => state.transfer(ActivePane::Middle, ActivePane::Right),
                ActivePane::Right => "no-op: no pane right of Personal".into(),
                ActivePane::Bottom => "no-op: Ignored has no right neighbor".into(),
            },
            (true, "ArrowDown") => match pane {
                ActivePane::Left | ActivePane::Middle | ActivePane::Right => {
                    state.transfer(pane, ActivePane::Bottom)
                }
                ActivePane::Bottom => "no-op: already in Ignored pane".into(),
            },
            (true, "ArrowUp") => match pane {
                ActivePane::Bottom => state.transfer(ActivePane::Bottom, ActivePane::Middle),
                _ => "no-op: Shift+Up only applies from Ignored pane".into(),
            },
            _ => return,
        };

        state.log(format!("{}  →  {}", prefix, detail));
    });

    // ── Window resize → save dimensions (debounced) ──────────────────────────
    thread_local! {
        static RESIZE_TIMEOUT: std::cell::RefCell<Option<leptos::prelude::TimeoutHandle>> = const { std::cell::RefCell::new(None) };
        static LAST_SAVED_SIZE: std::cell::Cell<(f64, f64)> = const { std::cell::Cell::new((0.0, 0.0)) };
    }

    let resize_handle = window_event_listener(ev::resize, move |_ev| {
        // Cancel any pending timeout to debounce the resize events
        RESIZE_TIMEOUT.with(|cell| {
            if let Some(handle) = cell.borrow_mut().take() {
                handle.clear();
            }
        });

        let handle = leptos::prelude::set_timeout_with_handle(
            move || {
                let Some(w) = web_sys::window() else { return };
                let width = w
                    .outer_width()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1024.0);
                let height = w
                    .outer_height()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(700.0);

                let changed = LAST_SAVED_SIZE.with(|cell| {
                    let last = cell.get();
                    if last != (width, height) {
                        cell.set((width, height));
                        true
                    } else {
                        false
                    }
                });

                if changed {
                    state.log(format!(
                        "[Window] resize (debounced) → {width:.0}×{height:.0} (saving…)"
                    ));

                    spawn_local(async move {
                        ipc::save_window_size(state, width, height).await;
                    });
                }
            },
            std::time::Duration::from_millis(200),
        );

        if let Ok(h) = handle {
            RESIZE_TIMEOUT.with(|cell| {
                *cell.borrow_mut() = Some(h);
            });
        }
    });

    on_cleanup(move || {
        drop(key_handle);
        drop(move_handle);
        drop(up_handle);
        drop(resize_handle);
    });

    // Mirror the drag signal to a global cursor so it stays correct off-handle.
    Effect::new(move || match state.drag.get() {
        None => set_drag_cursor(""),
        Some(drag) => {
            let cursor = match drag.target {
                DragTarget::LeftHandle | DragTarget::RightHandle => "col-resize",
                DragTarget::TopHandle | DragTarget::BottomHandle => "row-resize",
            };
            set_drag_cursor(cursor);
        }
    });

    view! {
        <div class="app-container">
            <Header />
            <div class="main-area">
                <div class="top-section">
                    <Pane pane_id=ActivePane::Left />
                    <ResizeHandle dir=ResizeDir::Horizontal target=DragTarget::LeftHandle />
                    <Pane pane_id=ActivePane::Middle />
                    <ResizeHandle dir=ResizeDir::Horizontal target=DragTarget::RightHandle />
                    <Pane pane_id=ActivePane::Right />
                </div>
                <ResizeHandle dir=ResizeDir::Vertical target=DragTarget::TopHandle />
                <Pane pane_id=ActivePane::Bottom is_bottom=true />
                {move || state.show_debug_log.get().then(|| view! {
                    <ResizeHandle dir=ResizeDir::Vertical target=DragTarget::BottomHandle />
                    <DebugLog />
                })}
            </div>

            // Mapping modal: rendered only while a pending mapping exists.
            {move || state.pending_mapping.get().map(|pm| view! { <MappingModal pm=pm /> })}

            // Month selection modal: rendered only while open.
            {move || state.is_month_modal_open.get().then(|| view! { <MonthModal /> })}

            // Renders the auto-assign modal when assigning an item.
            {move || state.assign_modal_item.get().map(|item| {
                let props = build_assign_modal_props(state, &item);
                let vendor = props.preview_vendor.clone();
                let description = props.preview_description.clone();
                let on_save = move |rule: hho_types::AutoAssignRule| {
                    let vendor = vendor.clone();
                    let description = description.clone();
                    spawn_local(async move {
                        save_rule(state, rule, &vendor, &description).await;
                    });
                };
                let on_cancel = move || {
                    state.assign_modal_item.set(None);
                };
                view! {
                    <RuleEditorModal
                        preview_vendor=props.preview_vendor
                        preview_description=props.preview_description
                        initial_vendor_regex=props.initial_vendor_regex
                        initial_description_regex=props.initial_description_regex
                        initial_pane=props.initial_pane
                        initial_category_override=props.initial_category_override
                        on_save=on_save
                        on_cancel=on_cancel
                    />
                }
            })}

            // Renders the transaction editing modal when editing an item.
            {move || state.editing_transaction_item.get().map(|item| {
                let tx_id = item.txn.id;
                let on_save = move |updated_txn: hho_types::Transaction| {
                    let year = state.selected_year.get_untracked();
                    let month = state.selected_month.get_untracked();
                    state.raw_transactions.update(|txns| {
                        if let Some(target) = txns.iter_mut().find(|t| t.id == tx_id) {
                            let mut next_txn = updated_txn.clone();
                            next_txn.id = tx_id;
                            *target = next_txn;
                        }
                    });

                    let txn_year: i32 = updated_txn.date[0..4].parse().unwrap_or(year);
                    let txn_month: i32 = updated_txn.date[5..7].parse().unwrap_or(month);
                    if txn_year != year || txn_month != month {
                        state.selected_year.set(txn_year);
                        state.selected_month.set(txn_month);
                    }

                    state.apply_month_filter();
                    state.editing_transaction_item.set(None);
                };
                let on_cancel = move || {
                    state.editing_transaction_item.set(None);
                };
                view! {
                    <TransactionEditorModal
                        item=item.clone()
                        on_save=on_save
                        on_cancel=on_cancel
                    />
                }
            })}

            // Renders the split transaction modal when active.
            {move || state.split_transaction_item.get().map(|item| {
                let tx_id = item.txn.id;
                let on_save = move |splits: Vec<(i64, String, hho_types::RulePane)>| {
                    if splits.is_empty() {
                        return;
                    }
                    state.raw_transactions.update(|txns| {
                        // Locate the original transaction in the raw transactions list by ID.
                        if let Some(pos) = txns.iter().position(|t| t.id == tx_id) {
                            let mut next_id = txns.iter().filter_map(|t| t.id).max().unwrap_or(0) + 1;
                            let base_txn = txns[pos].clone();
                            
                            // Process the first split portion in-place on the original transaction.
                            txns[pos].amount_cents = splits[0].0;
                            txns[pos].description = splits[0].1.clone();
                            txns[pos].manual_pane = Some(splits[0].2);
                            
                            // Create copy transactions for the remaining split portions.
                            #[allow(clippy::explicit_counter_loop)]
                            for split in splits.iter().skip(1) {
                                let mut new_txn = base_txn.clone();
                                new_txn.id = Some(next_id);
                                next_id += 1;
                                new_txn.amount_cents = split.0;
                                new_txn.description = split.1.clone();
                                new_txn.manual_pane = Some(split.2);
                                txns.push(new_txn);
                            }
                        }
                    });
                    state.apply_month_filter();
                    state.split_transaction_item.set(None);
                };
                let on_cancel = move || {
                    state.split_transaction_item.set(None);
                };
                view! {
                    <SplitTransactionModal
                        item=item.clone()
                        on_save=on_save
                        on_cancel=on_cancel
                    />
                }
            })}

            // Rules manager modal: rendered only while open.
            {move || state.is_rules_modal_open.get().then(|| view! { <RulesModal /> })}

            // Nicknames editor modal: rendered only while active.
            {move || state.nickname_modal_item.get().map(|item| {
                let vendor = item.txn.vendor.clone();
                let escaped_vendor = crate::logic::escape_regex(&vendor);
                let matched_rule = state.nickname_rules.get_untracked().into_iter().find(|r| r.regex == escaped_vendor);
                let initial_regex = matched_rule.as_ref().map(|r| r.regex.clone()).unwrap_or(escaped_vendor);
                let initial_nickname = matched_rule.as_ref().map(|r| r.nickname.clone()).unwrap_or_default();
                
                let on_save = move |rule: hho_types::NicknameRule| {
                    spawn_local(async move {
                        save_nickname_rule(state, rule).await;
                      });
                };
                let on_cancel = move || {
                    state.nickname_modal_item.set(None);
                };
                view! {
                    <NicknameEditorModal
                        preview_vendor=vendor
                        initial_regex=initial_regex
                        initial_nickname=initial_nickname
                        on_save=on_save
                        on_cancel=on_cancel
                    />
                }
            })}

            // Nicknames manager rules modal: rendered only while open.
            {move || state.is_nickname_manager_open.get().then(|| view! { <NicknameRulesModal /> })}

            // Renders the manual transaction creation modal when open.
            {move || state.is_create_transaction_modal_open.get().then(|| view! { <CreateTransactionModal /> })}

            // Print view component for physical printing or PDF export.
            <PrintView />

            // Footer status bar containing the show debug log toggle.
            <div class="footer-bar">
                <label class="footer-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || state.show_debug_log.get()
                        on:change=move |e| state.show_debug_log.set(event_target_checked(&e))
                    />
                    <span>"Show Debug Log"</span>
                </label>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matching_rule_returns_correct_indices() {
        let state = AppState::new();
        state.auto_assign_rules.set(vec![
            hho_types::AutoAssignRule {
                regex: Some("STARBUCKS.*".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: hho_types::RulePane::Joint,
                category_override: Some("Coffee".to_string()),
            },
            hho_types::AutoAssignRule {
                regex: Some("NETFLIX".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: hho_types::RulePane::Personal,
                category_override: None,
            },
        ]);

        // Verify matches are found with correct indices
        let match1 = find_matching_rule(state, "STARBUCKS COFFEE", "");
        assert!(match1.is_some());
        let (idx1, rule1) = match1.unwrap();
        assert_eq!(idx1, 0);
        assert_eq!(rule1.pane, hho_types::RulePane::Joint);

        let match2 = find_matching_rule(state, "NETFLIX", "");
        assert!(match2.is_some());
        let (idx2, rule2) = match2.unwrap();
        assert_eq!(idx2, 1);
        assert_eq!(rule2.pane, hho_types::RulePane::Personal);

        let match3 = find_matching_rule(state, "GOOGLE", "");
        assert!(match3.is_none());
    }

    #[test]
    fn test_build_assign_modal_props_populates_correctly() {
        let state = AppState::new();
        state.auto_assign_rules.set(vec![hho_types::AutoAssignRule {
            regex: Some("STARBUCKS.*".to_string()),
            vendor_regex: None,
            description_regex: None,
            pane: hho_types::RulePane::Joint,
            category_override: Some("Coffee".to_string()),
        }]);

        let item = Item {
            id: 1,
            label: "".to_string(),
            auto_matched: true,
            txn: crate::dto::Transaction {
                id: None,
                date: "2026-05-15".to_string(),
                vendor: "STARBUCKS COFFEE".to_string(),
                description: "Seattle branch".to_string(),
                category: "Uncategorized".to_string(),
                amount_cents: 100,
                direction: crate::dto::Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
        };

        // Verifies that properties populate correctly from matching rules.
        let props = build_assign_modal_props(state, &item);
        assert_eq!(props.preview_vendor, "STARBUCKS COFFEE");
        assert_eq!(props.preview_description, "Seattle branch");
        assert_eq!(props.initial_vendor_regex, "STARBUCKS.*");
        assert_eq!(props.initial_description_regex, "");
        assert_eq!(props.initial_pane, hho_types::RulePane::Joint);
        assert_eq!(props.initial_category_override, "Coffee");
    }

    #[test]
    fn test_modal_overlays_are_modal() {
        // Assert that none of the modal components allow closing the modal via clicking on the overlay.
        // This ensures the main window remains inaccessible and background clicks do not close the dialogs.
        let paths = [
            "src/components/rule_editor_modal.rs",
            "src/components/rules_modal.rs",
            "src/components/transaction_editor_modal.rs",
            "src/components/create_transaction_modal.rs",
            "src/components/mapping_modal.rs",
            "src/components/month_modal.rs",
        ];

        // Regex pattern to find any `<div class="...modal-overlay..." ... on:click=...>`
        let re = regex::Regex::new(r#"(?s)<div\s+class="[^"]*modal-overlay[^"]*"[^>]*on:click\s*="#).unwrap();

        for path in &paths {
            let content = std::fs::read_to_string(path)
                .unwrap_or_else(|_| panic!("Failed to read {}", path));

            if re.is_match(&content) {
                panic!("Security/UX Bug: Modal component {} allows closing by clicking on the overlay backdrop! All dialog overlays must block interactions and not dismiss on background clicks.", path);
            }
        }
    }
}
