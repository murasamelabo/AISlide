#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod requests;
use requests::RequestGate;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

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

#[tauri::command]
async fn save_project(window: tauri::WebviewWindow, gate: tauri::State<'_, RequestGate>, document: aislide_core::document::Document, operation_id: String) -> Result<Option<aislide_core::publication::PublishedProject>, String> {
    let guard = gate.begin(window.label(), &operation_id)?;
    let app = window.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        aislide_core::document::verify(&document).map_err(|error| error.to_string())?;
        let selected = app.dialog().file().set_file_name("report.pptx").add_filter("PowerPoint presentation and checkpoint", &["pptx"]).blocking_save_file();
        let Some(selected) = selected else { return Ok(None) };
        let path = selected.into_path().map_err(|_| "Only local filesystem output is supported".to_string())?;
        aislide_core::publication::publish_project(&document, &path).map(Some).map_err(|error| error.to_string())
    }).await.map_err(|_| "Project save worker failed".to_string())?
}

#[tauri::command]
async fn save_presentation(window: tauri::WebviewWindow, gate: tauri::State<'_, RequestGate>, document: aislide_core::document::Document, operation_id: String, filename: Option<String>) -> Result<Option<aislide_core::publication::PublishedPresentation>, String> {
    let guard = gate.begin(window.label(), &operation_id)?;
    let app = window.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        aislide_core::document::verify(&document).map_err(|error| error.to_string())?;
        let name = save_filename(&document.deck.title, filename.as_deref())?;
        let selected = app.dialog().file().set_file_name(name).add_filter("New Open XML PowerPoint presentation", &["pptx"]).blocking_save_file();
        let Some(selected) = selected else { return Ok(None) };
        let path = selected.into_path().map_err(|_| "Only local filesystem output is supported".to_string())?;
        aislide_core::publication::publish_presentation(&document, &path).map(Some).map_err(|error| error.to_string())
    }).await.map_err(|_| "Presentation save worker failed".to_string())?
}

fn save_filename(title: &str, requested: Option<&str>) -> Result<String, String> {
    let name = match requested {
        Some(name) => name.trim().to_owned(),
        None => format!("AISlide-{}.pptx", title.chars().filter(|character| !character.is_control() && !"\\/:*?\"<>|".contains(*character)).take(80).collect::<String>().trim()),
    };
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_lowercase();
    let reserved = ["con", "prn", "aux", "nul"].contains(&stem.as_str()) || (1..=9).any(|index| stem == format!("com{index}") || stem == format!("lpt{index}"));
    if name.starts_with('.') || !name.to_ascii_lowercase().ends_with(".pptx") || name.chars().count() > 180 || name.chars().any(|character| character.is_control() || "\\/:*?\"<>|".contains(character)) || reserved {
        return Err("Choose a plain PPTX filename without path characters or reserved names".into());
    }
    Ok(name)
}

fn main() {
    let result = tauri::Builder::default()
        .manage(RequestGate::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![core_request, cancel_core_request, save_project, save_presentation])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("AISlide failed to start: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn save_filename_preserves_requested_names_and_rejects_paths() {
        assert_eq!(super::save_filename("Fallback", Some("Design review.pptx")).unwrap(), "Design review.pptx");
        assert_eq!(super::save_filename("Fallback", None).unwrap(), "AISlide-Fallback.pptx");
        for name in ["../report.pptx", "C:\\report.pptx", "CON.pptx", ".pptx", "report.txt", "report.pptx:other", "bad\nname.pptx"] {
            assert!(super::save_filename("Fallback", Some(name)).is_err(), "{name}");
        }
    }
}