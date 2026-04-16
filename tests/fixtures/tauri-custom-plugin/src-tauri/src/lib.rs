pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(crate::commands::patient::plugin())
        .invoke_handler(tauri::generate_handler![commands::patient::get_patient])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
