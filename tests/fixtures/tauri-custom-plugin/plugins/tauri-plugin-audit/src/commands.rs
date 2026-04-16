use tauri::{command, ipc::Channel, AppHandle, Emitter, Runtime, Window};

#[command]
pub async fn upload<R: Runtime>(
    app: AppHandle<R>,
    _window: Window<R>,
    on_progress: Channel<u32>,
    url: String,
) -> Result<(), String> {
    on_progress.send(50).map_err(|e| e.to_string())?;
    app.emit("audit-upload-complete", url)
        .map_err(|e| e.to_string())?;
    Ok(())
}
