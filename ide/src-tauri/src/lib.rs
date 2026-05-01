use ail_ui_bridge::{get_handler, new_bridge_state};

pub fn run() {
    tauri::Builder::default()
        .manage(new_bridge_state())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(get_handler())
        .run(tauri::generate_context!())
        .expect("error while running AIL IDE");
}
