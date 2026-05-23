use pit2sop_core::Pit2Sop;
use pit2sop_core::models::{
    AiHealthCheck, AppStatus, DesktopError, DesktopSettings, DoingMatch, PatchActionSummary,
    PendingPatchSummary, ProcessingSummary, SaveAiSecretInput, SaveSettingsInput, SearchResult,
    SecretSaveSummary,
};
use std::path::{Path, PathBuf};
use std::process::Command;

fn load_app() -> Result<Pit2Sop, DesktopError> {
    Pit2Sop::load().map_err(desktop_error)
}

#[tauri::command]
fn app_status() -> Result<AppStatus, DesktopError> {
    load_app()?.status().map_err(desktop_error)
}

#[tauri::command]
fn get_settings() -> Result<DesktopSettings, DesktopError> {
    Ok(load_app()?.settings())
}

#[tauri::command]
fn save_settings(input: SaveSettingsInput) -> Result<AppStatus, DesktopError> {
    Pit2Sop::save_settings(input).map_err(desktop_error)
}

#[tauri::command]
fn save_ai_secret(input: SaveAiSecretInput) -> Result<SecretSaveSummary, DesktopError> {
    Pit2Sop::save_ai_secret(input).map_err(desktop_error)
}

#[tauri::command]
fn clear_ai_secret(provider: String) -> Result<SecretSaveSummary, DesktopError> {
    Pit2Sop::clear_ai_secret(&provider).map_err(desktop_error)
}

#[tauri::command]
fn test_ai_provider() -> Result<AiHealthCheck, DesktopError> {
    Ok(load_app()?.test_ai_provider())
}

#[tauri::command]
fn open_vault() -> Result<(), DesktopError> {
    let app = load_app()?;
    open_path(&app.config.vault_path)
}

#[tauri::command]
fn process_pit(text: String) -> Result<ProcessingSummary, DesktopError> {
    load_app()?.process_pit(&text).map_err(desktop_error)
}

#[tauri::command]
fn doing(text: String) -> Result<Vec<DoingMatch>, DesktopError> {
    load_app()?.doing(&text).map_err(desktop_error)
}

#[tauri::command]
fn search(query: String) -> Result<Vec<SearchResult>, DesktopError> {
    load_app()?.search(&query).map_err(desktop_error)
}

#[tauri::command]
fn pending_patches() -> Result<Vec<PendingPatchSummary>, DesktopError> {
    load_app()?.pending_patches().map_err(desktop_error)
}

#[tauri::command]
fn apply_pending_patch(path: String) -> Result<PatchActionSummary, DesktopError> {
    load_app()?
        .apply_pending_patch(&PathBuf::from(path))
        .map_err(desktop_error)
}

#[tauri::command]
fn reject_pending_patch(path: String) -> Result<PatchActionSummary, DesktopError> {
    load_app()?
        .reject_pending_patch(&PathBuf::from(path))
        .map_err(desktop_error)
}

#[cfg(target_os = "macos")]
fn open_path(path: &Path) -> Result<(), DesktopError> {
    run_open_command(Command::new("open").arg(path))
}

#[cfg(target_os = "windows")]
fn open_path(path: &Path) -> Result<(), DesktopError> {
    let path = path.to_string_lossy().to_string();
    run_open_command(Command::new("cmd").args(["/C", "start", ""]).arg(path))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_path(path: &Path) -> Result<(), DesktopError> {
    run_open_command(Command::new("xdg-open").arg(path))
}

fn run_open_command(command: &mut Command) -> Result<(), DesktopError> {
    let status = command
        .status()
        .map_err(|error| DesktopError::new("filesystem", error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(DesktopError::new(
            "filesystem",
            format!("failed to open path, status: {}", status),
        ))
    }
}

fn desktop_error(error: impl std::fmt::Display) -> DesktopError {
    let message = error.to_string();
    DesktopError::new(classify_error(&message), message)
}

fn classify_error(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("api key") || lower.contains("secret") {
        "secret"
    } else if lower.contains("vault path")
        || lower.contains("unsupported language")
        || lower.contains("unsupported ai provider")
        || lower.contains("model")
        || lower.contains("base url")
        || lower.contains("must not be empty")
    {
        "validation"
    } else if lower.contains("deepseek") || lower.contains("ai ") || lower.contains("provider") {
        "ai"
    } else if lower.contains("database") || lower.contains("sqlite") {
        "database"
    } else if lower.contains("config") {
        "config"
    } else if lower.contains("permission")
        || lower.contains("readable")
        || lower.contains("writable")
        || lower.contains("failed to read")
        || lower.contains("failed to write")
        || lower.contains("file")
        || lower.contains("directory")
        || lower.contains("path")
    {
        "filesystem"
    } else {
        "unknown"
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            app_status,
            get_settings,
            save_settings,
            save_ai_secret,
            clear_ai_secret,
            test_ai_provider,
            open_vault,
            process_pit,
            doing,
            search,
            pending_patches,
            apply_pending_patch,
            reject_pending_patch
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
