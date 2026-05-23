use pit2sop_core::Pit2Sop;
use pit2sop_core::models::{
    AiHealthCheck, AppStatus, DesktopError, DesktopSettings, DoingMatch, PatchActionSummary,
    PendingPatchSummary, ProcessingSummary, SaveAiSecretInput, SaveSettingsInput, SearchResult,
    SecretSaveSummary,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize)]
struct AppVersion {
    version: String,
    git_sha: String,
}

#[derive(Debug, Serialize)]
struct PendingPatchDetail {
    path: String,
    target: String,
    source_pit: String,
    status: String,
    title: String,
    checklist_items: Vec<String>,
    body: String,
}

#[derive(Debug, Deserialize)]
struct PendingPatchFrontmatter {
    #[serde(rename = "type")]
    doc_type: Option<String>,
    target: Option<String>,
    source_pit: Option<String>,
    status: Option<String>,
    title: Option<String>,
}

fn load_app() -> Result<Pit2Sop, DesktopError> {
    Pit2Sop::load().map_err(desktop_error)
}

#[tauri::command]
fn app_version() -> AppVersion {
    AppVersion {
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_sha: option_env!("PIT2SOP_GIT_SHA")
            .unwrap_or("unknown")
            .to_string(),
    }
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
fn open_markdown_path(path: String) -> Result<(), DesktopError> {
    let app = load_app()?;
    let path = resolve_path_inside_vault(&app.config.vault_path, &path)?;
    open_path(&path)
}

#[tauri::command]
fn reveal_markdown_path(path: String) -> Result<(), DesktopError> {
    let app = load_app()?;
    let path = resolve_path_inside_vault(&app.config.vault_path, &path)?;
    reveal_path(&path)
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
fn pending_patch_detail(path: String) -> Result<PendingPatchDetail, DesktopError> {
    let app = load_app()?;
    let path = resolve_pending_patch_path(&app.config.vault_path, &path)?;
    let content = fs::read_to_string(&path).map_err(desktop_error)?;
    let (yaml, body) = split_frontmatter(&content)
        .ok_or_else(|| DesktopError::new("validation", "pending patch is missing frontmatter"))?;
    let frontmatter: PendingPatchFrontmatter = serde_yaml::from_str(yaml).map_err(desktop_error)?;
    if frontmatter.doc_type.as_deref() != Some("pending_patch") {
        return Err(DesktopError::new(
            "validation",
            "file is not a pending patch",
        ));
    }
    Ok(PendingPatchDetail {
        path: path_relative_to(&app.config.vault_path, &path),
        target: frontmatter.target.unwrap_or_default(),
        source_pit: frontmatter.source_pit.unwrap_or_default(),
        status: frontmatter
            .status
            .unwrap_or_else(|| "needs_review".to_string()),
        title: frontmatter
            .title
            .unwrap_or_else(|| file_stem(&path).to_string()),
        checklist_items: extract_checklist_items(body),
        body: body.trim().to_string(),
    })
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

#[tauri::command]
fn diagnostics(
    last_error_kind: Option<String>,
    last_error_message: Option<String>,
) -> Result<String, DesktopError> {
    let version = app_version();
    let app = load_app()?;
    let settings = app.settings();
    let status = app.status();
    let pending_count = app.pending_patches().map(|items| items.len());
    let mut lines = vec![
        format!("Pit2SOP {}", version.version),
        format!("commit: {}", version.git_sha),
        format!(
            "platform: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        format!("vault_path: {}", app.config.vault_path.display()),
        format!("config_saved: {}", settings.config_saved),
        format!("vault_exists: {}", settings.vault_exists),
        format!("vault_initialized: {}", settings.vault_initialized),
        format!("vault_writable: {}", settings.vault_writable),
        format!("ai_provider: {}", app.config.ai.provider),
        format!("ai_model: {}", app.config.ai.model),
        format!("api_key_configured: {}", settings.has_deepseek_api_key),
    ];
    match status {
        Ok(status) => {
            lines.push(format!("db_path: {}", status.db_path.display()));
            lines.push(format!("indexed_docs: {}", status.indexed_docs));
            lines.push(format!("pit_files: {}", status.pit_files));
            lines.push(format!("sop_files: {}", status.sop_files));
        }
        Err(error) => lines.push(format!("status_error: {}", error)),
    }
    match pending_count {
        Ok(count) => lines.push(format!("pending_count: {}", count)),
        Err(error) => lines.push(format!("pending_error: {}", error)),
    }
    lines.push(format!(
        "last_error_kind: {}",
        last_error_kind
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("none")
    ));
    lines.push(format!(
        "last_error_message: {}",
        last_error_message
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("none")
    ));
    Ok(lines.join("\n"))
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

#[cfg(target_os = "macos")]
fn reveal_path(path: &Path) -> Result<(), DesktopError> {
    run_open_command(Command::new("open").arg("-R").arg(path))
}

#[cfg(target_os = "windows")]
fn reveal_path(path: &Path) -> Result<(), DesktopError> {
    run_open_command(Command::new("explorer").arg("/select,").arg(path))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_path(path: &Path) -> Result<(), DesktopError> {
    open_path(path.parent().unwrap_or(path))
}

fn resolve_path_inside_vault(vault_path: &Path, input: &str) -> Result<PathBuf, DesktopError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(DesktopError::new("validation", "path must not be empty"));
    }
    let path = PathBuf::from(input);
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(DesktopError::new(
            "validation",
            "path must stay inside vault",
        ));
    }
    let candidate = if path.is_absolute() {
        path
    } else {
        vault_path.join(path)
    };
    let canonical_vault = vault_path.canonicalize().map_err(desktop_error)?;
    let canonical_path = candidate.canonicalize().map_err(desktop_error)?;
    if !canonical_path.starts_with(&canonical_vault) {
        return Err(DesktopError::new(
            "validation",
            "path must stay inside vault",
        ));
    }
    Ok(canonical_path)
}

fn resolve_pending_patch_path(vault_path: &Path, input: &str) -> Result<PathBuf, DesktopError> {
    let path = resolve_path_inside_vault(vault_path, input)?;
    let pending_root = vault_path
        .join("99_System/Pending Patches")
        .canonicalize()
        .map_err(desktop_error)?;
    if !path.starts_with(&pending_root) {
        return Err(DesktopError::new(
            "validation",
            "pending patch must be inside vault pending patch directory",
        ));
    }
    Ok(path)
}

fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some((&rest[..end], &rest[end + "\n---".len()..]))
}

fn extract_checklist_items(body: &str) -> Vec<String> {
    body.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- [ ]") || line.starts_with("- [x]"))
        .map(|line| {
            line.trim_start_matches("- [ ]")
                .trim_start_matches("- [x]")
                .split("<!--")
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .filter(|item| !item.is_empty())
        .collect()
}

fn path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn file_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Untitled pending patch")
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
            app_version,
            app_status,
            get_settings,
            save_settings,
            save_ai_secret,
            clear_ai_secret,
            test_ai_provider,
            open_vault,
            open_markdown_path,
            reveal_markdown_path,
            process_pit,
            doing,
            search,
            pending_patches,
            pending_patch_detail,
            apply_pending_patch,
            reject_pending_patch,
            diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
