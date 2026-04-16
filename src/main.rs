fn main() {
    if let Err(error) = source_map_tauri::run() {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}
