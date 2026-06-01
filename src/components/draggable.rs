use leptos::prelude::*;
use leptos::ev;

pub fn use_draggable() -> (Memo<String>, impl Fn(web_sys::MouseEvent) + Clone) {
    let (x_offset, set_x_offset) = signal(0.0);
    let (y_offset, set_y_offset) = signal(0.0);
    let (is_dragging, set_is_dragging) = signal(false);
    let (start_mouse, set_start_mouse) = signal((0.0, 0.0));
    let (start_offset, set_start_offset) = signal((0.0, 0.0));

    let on_mousedown = move |ev: web_sys::MouseEvent| {
        if ev.button() == 0 {
            set_is_dragging.set(true);
            set_start_mouse.set((ev.client_x() as f64, ev.client_y() as f64));
            set_start_offset.set((x_offset.get_untracked(), y_offset.get_untracked()));
            ev.prevent_default();
        }
    };

    let move_handle = window_event_listener(ev::mousemove, move |ev: web_sys::MouseEvent| {
        if is_dragging.get_untracked() {
            let (start_mx, start_my) = start_mouse.get_untracked();
            let (start_ox, start_oy) = start_offset.get_untracked();
            let dx = ev.client_x() as f64 - start_mx;
            let dy = ev.client_y() as f64 - start_my;
            set_x_offset.set(start_ox + dx);
            set_y_offset.set(start_oy + dy);
        }
    });

    let up_handle = window_event_listener(ev::mouseup, move |_ev| {
        set_is_dragging.set(false);
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
