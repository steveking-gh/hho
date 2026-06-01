use leptos::prelude::*;
use leptos::ev;
use std::rc::Rc;
use std::cell::Cell;

pub fn use_draggable() -> (Memo<String>, impl Fn(web_sys::MouseEvent) + Clone) {
    let (x_offset, set_x_offset) = signal(0.0);
    let (y_offset, set_y_offset) = signal(0.0);
    
    let is_dragging = Rc::new(Cell::new(false));
    let start_mouse = Rc::new(Cell::new((0.0, 0.0)));
    let start_offset = Rc::new(Cell::new((0.0, 0.0)));

    let is_dragging_mousedown = Rc::clone(&is_dragging);
    let start_mouse_mousedown = Rc::clone(&start_mouse);
    let start_offset_mousedown = Rc::clone(&start_offset);

    let on_mousedown = move |ev: web_sys::MouseEvent| {
        if ev.button() == 0 {
            is_dragging_mousedown.set(true);
            start_mouse_mousedown.set((ev.client_x() as f64, ev.client_y() as f64));
            start_offset_mousedown.set((x_offset.get_untracked(), y_offset.get_untracked()));
            ev.prevent_default();
        }
    };

    let is_dragging_mousemove = Rc::clone(&is_dragging);
    let start_mouse_mousemove = Rc::clone(&start_mouse);
    let start_offset_mousemove = Rc::clone(&start_offset);

    let move_handle = window_event_listener(ev::mousemove, move |ev: web_sys::MouseEvent| {
        if is_dragging_mousemove.get() {
            let (start_mx, start_my) = start_mouse_mousemove.get();
            let (start_ox, start_oy) = start_offset_mousemove.get();
            let dx = ev.client_x() as f64 - start_mx;
            let dy = ev.client_y() as f64 - start_my;
            set_x_offset.set(start_ox + dx);
            set_y_offset.set(start_oy + dy);
        }
    });

    let is_dragging_mouseup = Rc::clone(&is_dragging);
    let up_handle = window_event_listener(ev::mouseup, move |_ev| {
        is_dragging_mouseup.set(false);
    });

    on_cleanup(move || {
        drop(move_handle);
        drop(up_handle);
    });

    let style = Memo::new(move |_| {
        format!("transform: translate({:.0}px, {:.0}px);", x_offset.get(), y_offset.get())
    });

    (style, on_mousedown)
}
