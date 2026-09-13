#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod requests;
use requests::RequestGate;

#[tauri::command]
async fn core_request(window: tauri::WebviewWindow, gate: tauri::State<'_, RequestGate>, request: serde_json::Value, operation_id: Option<String>) -> Result<serde_json::Value, String> {
    let guard = gate.begin(window.label(), operation_id.as_deref().unwrap_or("legacy"))?;
    aislide_core::protocol::execute_request_async(request, guard.cancellation()).await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_core_request(window: tauri::WebviewWindow, gate: tauri::State<'_, RequestGate>, operation_id: String) -> Result<bool, String> {
    gate.cancel(window.label(), &operation_id)
}

fn main() {
    let result = tauri::Builder::default()
        .manage(RequestGate::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![core_request, cancel_core_request])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("AISlide failed to start: {error}");
        std::process::exit(1);
    }
}