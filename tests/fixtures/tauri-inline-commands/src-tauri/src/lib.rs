#[tauri::command]
async fn get_all_accounts() -> Result<Vec<String>, String> {
  Ok(vec![])
}

#[tauri::command]
async fn open_devtools() -> Result<(), String> {
  Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![get_all_accounts, open_devtools])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
