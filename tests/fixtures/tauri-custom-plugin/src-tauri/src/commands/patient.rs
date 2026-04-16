use tauri::{command, State};

pub struct AppState;
pub struct PatientDto;
pub struct CommandError;

#[command]
pub async fn get_patient(
    _state: State<'_, AppState>,
    patient_id: String,
) -> Result<PatientDto, CommandError> {
    let _ = patient_id;
    Ok(PatientDto)
}

pub fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("patient")
        .invoke_handler(tauri::generate_handler![get_patient])
        .build()
}
