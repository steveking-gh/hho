// Tauri application entry point.
// Builds the native OS menu (File > Quit) and wires the menu event handler.

use tauri::menu::{Menu, MenuItem, Submenu};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Build native menu: File → Quit.
            // MenuId is cloned before the builder consumes quit_item so the
            // event handler closure can match against it without holding a
            // reference to the item itself.
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let quit_id   = quit_item.id().clone();
            let file_menu = Submenu::with_items(app, "File", true, &[&quit_item])?;
            let menu      = Menu::with_items(app, &[&file_menu])?;
            app.set_menu(menu)?;

            app.on_menu_event(move |app_handle, event| {
                if event.id() == &quit_id {
                    app_handle.exit(0);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
