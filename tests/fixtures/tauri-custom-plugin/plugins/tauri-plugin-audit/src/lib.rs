use tauri::{plugin::Builder, Emitter, Listener, Runtime};

mod commands;

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    Builder::new("audit")
        .setup(|app, _api| {
            app.emit("audit-plugin-ready", ()).ok();
            Ok(())
        })
        .on_webview_ready(|window| {
            window.listen("content-loaded", |_| {});
        })
        .invoke_handler(tauri::generate_handler![commands::upload])
        .build()
}
