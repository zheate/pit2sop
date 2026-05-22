use pit2sop_core::Pit2Sop;
use pit2sop_core::models::{
    AiHealthCheck, AppStatus, DesktopSettings, DoingMatch, PatchActionSummary, PendingPatchSummary,
    ProcessingSummary, SaveAiSecretInput, SaveSettingsInput, SearchResult, SecretSaveSummary,
};
use std::path::{Path, PathBuf};
use std::process::Command;

fn load_app() -> Result<Pit2Sop, String> {
    Pit2Sop::load().map_err(|error| error.to_string())
}

#[tauri::command]
fn app_status() -> Result<AppStatus, String> {
    load_app()?.status().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_settings() -> Result<DesktopSettings, String> {
    Ok(load_app()?.settings())
}

#[tauri::command]
fn save_settings(input: SaveSettingsInput) -> Result<AppStatus, String> {
    Pit2Sop::save_settings(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_ai_secret(input: SaveAiSecretInput) -> Result<SecretSaveSummary, String> {
    Pit2Sop::save_ai_secret(input).map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_ai_secret(provider: String) -> Result<SecretSaveSummary, String> {
    Pit2Sop::clear_ai_secret(&provider).map_err(|error| error.to_string())
}

#[tauri::command]
fn test_ai_provider() -> Result<AiHealthCheck, String> {
    Ok(load_app()?.test_ai_provider())
}

#[tauri::command]
fn open_vault() -> Result<(), String> {
    let app = load_app()?;
    open_path(&app.config.vault_path)
}

#[tauri::command]
fn process_pit(text: String) -> Result<ProcessingSummary, String> {
    load_app()?
        .process_pit(&text)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn doing(text: String) -> Result<Vec<DoingMatch>, String> {
    load_app()?.doing(&text).map_err(|error| error.to_string())
}

#[tauri::command]
fn search(query: String) -> Result<Vec<SearchResult>, String> {
    load_app()?
        .search(&query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn pending_patches() -> Result<Vec<PendingPatchSummary>, String> {
    load_app()?
        .pending_patches()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_pending_patch(path: String) -> Result<PatchActionSummary, String> {
    load_app()?
        .apply_pending_patch(&PathBuf::from(path))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn reject_pending_patch(path: String) -> Result<PatchActionSummary, String> {
    load_app()?
        .reject_pending_patch(&PathBuf::from(path))
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn open_path(path: &Path) -> Result<(), String> {
    run_open_command(Command::new("open").arg(path))
}

#[cfg(target_os = "windows")]
fn open_path(path: &Path) -> Result<(), String> {
    let path = path.to_string_lossy().to_string();
    run_open_command(Command::new("cmd").args(["/C", "start", ""]).arg(path))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_path(path: &Path) -> Result<(), String> {
    run_open_command(Command::new("xdg-open").arg(path))
}

fn run_open_command(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to open path, status: {}", status))
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
