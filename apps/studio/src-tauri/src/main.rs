#![cfg_attr(all(target_os = "windows", not(test)), windows_subsystem = "windows")]

mod requests;
use requests::RequestGate;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
struct RecoveryGate(RequestGate);

struct RecoveryRoot(std::path::PathBuf);

#[cfg(debug_assertions)]
fn private_recovery_root(path: &std::path::Path, owner: &str) -> Result<std::path::PathBuf, String> {
    if !path.is_absolute() || owner.len() != 36 || !owner.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'-') {
        return Err("Invalid owned native test recovery directory".into());
    }
    let root = path.canonicalize().map_err(|error| error.to_string())?;
    let temporary = std::env::temp_dir().canonicalize().map_err(|error| error.to_string())?;
    if root.parent() != Some(temporary.as_path()) || !root.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("aislide-native-owned-")) {
        return Err("Native test recovery must be a direct owned temporary directory".into());
    }
    let marker = root.join(".aislide-test-owner");
    let metadata = std::fs::symlink_metadata(&marker).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() != 36 || std::fs::read_to_string(marker).map_err(|error| error.to_string())? != owner {
        return Err("Native test recovery ownership marker does not match".into());
    }
    Ok(root.join("recovery-v2"))
}

fn recovery_root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    if let Some(path) = std::env::var_os("AISLIDE_TEST_RECOVERY_ROOT") {
        #[cfg(debug_assertions)]
        { return private_recovery_root(std::path::Path::new(&path), &std::env::var("AISLIDE_TEST_RECOVERY_OWNER").unwrap_or_default()); }
        #[cfg(not(debug_assertions))]
        { let _ = path; return Err("Private native test recovery requires a debug build".into()); }
    }
    app.path().app_local_data_dir().map(|root| root.join("recovery-v2")).map_err(|error| error.to_string())
}

#[tauri::command]
async fn recovery_request(window: tauri::WebviewWindow, gate: tauri::State<'_, RecoveryGate>, root: tauri::State<'_, RecoveryRoot>, request: serde_json::Value, operation_id: String) -> Result<serde_json::Value, String> {
    let guard = gate.0.begin(window.label(), &operation_id)?;
    let root = root.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        aislide_core::recovery::storage::execute(&root, request, &guard.cancellation()).map_err(|error|error.to_string())
    }).await.map_err(|_|"Recovery worker failed; last checkpoint retained".to_string())?
}

#[tauri::command]
fn cancel_recovery_request(window: tauri::WebviewWindow, gate: tauri::State<'_, RecoveryGate>, operation_id: String) -> Result<bool, String> {
    gate.0.cancel(window.label(), &operation_id)
}

#[tauri::command]
async fn core_request(window: tauri::WebviewWindow, gate: tauri::State<'_, RequestGate>, request: serde_json::Value, operation_id: Option<String>) -> Result<serde_json::Value, String> {
    let guard = gate.begin_request(window.label(), operation_id.as_deref().unwrap_or("legacy"), &request)?;
    guard.execute_owned(request).await
}

#[tauri::command]
fn cancel_core_request(window: tauri::WebviewWindow, gate: tauri::State<'_, RequestGate>, operation_id: String) -> Result<bool, String> {
    gate.cancel(window.label(), &operation_id)
}

#[tauri::command]
async fn save_project(window: tauri::WebviewWindow, gate: tauri::State<'_, RequestGate>, mut document: aislide_core::document::Document, operation_id: String, capacity_profile: Option<aislide_core::limits::CapacityProfile>) -> Result<Option<aislide_core::publication::PublishedProject>, String> {
    document.capacity_profile = capacity_profile.unwrap_or_default();
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
async fn save_presentation(window: tauri::WebviewWindow, gate: tauri::State<'_, RequestGate>, mut document: aislide_core::document::Document, operation_id: String, filename: Option<String>, capacity_profile: Option<aislide_core::limits::CapacityProfile>) -> Result<Option<aislide_core::publication::PublishedPresentation>, String> {
    document.capacity_profile = capacity_profile.unwrap_or_default();
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
        .manage(RecoveryGate::default())
        .setup(|app| {
            app.manage(RecoveryRoot(recovery_root(app.handle()).map_err(std::io::Error::other)?));
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![core_request, cancel_core_request, save_project, save_presentation, recovery_request, cancel_recovery_request])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("AISlide failed to start: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(debug_assertions)]
    fn recovery_test_root_requires_owned_temporary_marker() {
        let owner = "12345678-1234-1234-1234-123456789abc";
        let name = format!("aislide-native-owned-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let root = std::env::temp_dir().join(name);
        std::fs::create_dir(&root).unwrap();
        assert!(super::private_recovery_root(&root, owner).is_err());
        std::fs::write(root.join(".aislide-test-owner"), owner).unwrap();
        assert_eq!(super::private_recovery_root(&root, owner).unwrap(), root.canonicalize().unwrap().join("recovery-v2"));
        assert!(super::private_recovery_root(&root, "00000000-0000-0000-0000-000000000000").is_err());
        assert!(super::private_recovery_root(std::path::Path::new("relative"), owner).is_err());
        assert!(super::private_recovery_root(&std::env::temp_dir(), owner).is_err());
        let nested = root.join("aislide-native-owned-nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join(".aislide-test-owner"), owner).unwrap();
        assert!(super::private_recovery_root(&nested, owner).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn save_filename_preserves_requested_names_and_rejects_paths() {
        assert_eq!(super::save_filename("Fallback", Some("Design review.pptx")).unwrap(), "Design review.pptx");
        assert_eq!(super::save_filename("Fallback", None).unwrap(), "AISlide-Fallback.pptx");
        for name in ["../report.pptx", "C:\\report.pptx", "CON.pptx", ".pptx", "report.txt", "report.pptx:other", "bad\nname.pptx"] {
            assert!(super::save_filename("Fallback", Some(name)).is_err(), "{name}");
        }
    }
}