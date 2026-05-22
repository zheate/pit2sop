use crate::models::{
    CaptureStatus, PatchActionSummary, PendingPatchSummary, RiskLevel, SopSummary,
};
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

pub const AUTO_ITEMS_START: &str = "<!-- pit2sop:start:auto-items -->";
pub const AUTO_ITEMS_END: &str = "<!-- pit2sop:end:auto-items -->";

const VAULT_DIRS: &[&str] = &[
    "00_Inbox/Mobile Captures",
    "00_Inbox/Desktop Captures",
    "00_Inbox/Raw Logs",
    "00_Inbox/Unprocessed",
    "01_Pits",
    "02_SOPs/Development",
    "02_SOPs/Release",
    "02_SOPs/Client Delivery",
    "02_SOPs/Personal Workflow",
    "03_Scenes",
    "04_Reviews/Weekly",
    "04_Reviews/Monthly",
    "04_Reviews/Executions",
    "90_Attachments/Audio",
    "90_Attachments/Images",
    "90_Attachments/Screenshots",
    "90_Attachments/Logs",
    "99_System/Templates",
    "99_System/Indexes",
    "99_System/Pending Patches",
    "99_System/Pending Patches/Applied",
    "99_System/Pending Patches/Rejected",
];

#[derive(Debug, Clone)]
pub struct PitMarkdownInput {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub source: String,
    pub status: String,
    pub scenario: String,
    pub risk: RiskLevel,
    pub recurrence: RiskLevel,
    pub sop_title: Option<String>,
    pub tags: Vec<String>,
    pub raw_text: String,
    pub symptom: String,
    pub root_cause: String,
    pub fix: String,
    pub prevention_rule: String,
    pub checklist_items: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SopMarkdownInput {
    pub id: String,
    pub title: String,
    pub category: String,
    pub scenario: String,
    pub risk: RiskLevel,
    pub triggers: Vec<String>,
    pub related_pit_title: String,
    pub related_pit_id: String,
    pub related_pit_note_stem: String,
    pub checklist_items: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum SopWriteOutcome {
    Created { path: PathBuf },
    Updated { path: PathBuf },
    PendingPatch { path: PathBuf, target: PathBuf },
}

#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub doc_id: String,
    pub doc_type: String,
    pub title: String,
    pub path: String,
    pub body: String,
}

pub fn init_vault_dirs(vault_path: &Path) -> Result<()> {
    for dir in VAULT_DIRS {
        fs::create_dir_all(vault_path.join(dir))
            .with_context(|| format!("failed to create vault dir {}", dir))?;
    }

    let agent_config = vault_path.join("99_System/Agent Config.md");
    if !agent_config.exists() {
        atomic_write(
            &agent_config,
            r#"---
type: pit2sop_config
version: 1
---

# Agent Config

## 默认规则

- CLI 输出语言跟随 `~/.pit2sop/config.toml`
- Obsidian Markdown 是唯一真相
- SQLite 只作为可重建缓存
"#,
        )?;
    }

    Ok(())
}

pub fn write_pit(vault_path: &Path, input: &PitMarkdownInput) -> Result<PathBuf> {
    let year = input.created_at.year();
    let dir = vault_path.join("01_Pits").join(year.to_string());
    fs::create_dir_all(&dir)?;
    let filename = format!("{}.md", pit_note_stem(input));
    let path = dir.join(filename);
    let relative_path = path_relative_to(vault_path, &path);
    let sop_link = input
        .sop_title
        .as_ref()
        .map(|title| obsidian_link(&sop_note_stem(title), title));
    let checklist = if input.checklist_items.is_empty() {
        "- [ ] 需要人工补充可执行检查项".to_string()
    } else {
        input
            .checklist_items
            .iter()
            .map(|item| format!("- [ ] {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let frontmatter = PitFrontmatter {
        doc_type: "pit",
        id: input.id.clone(),
        created: input.created_at.to_rfc3339(),
        source: input.source.clone(),
        status: input.status.clone(),
        scenario: input.scenario.clone(),
        risk: input.risk.as_str().to_string(),
        recurrence: input.recurrence.as_str().to_string(),
        sop: sop_link,
        tags: normalize_tags(&input.tags),
    };
    let yaml = serde_yaml::to_string(&frontmatter)?;

    let content = format!(
        r#"---
{yaml}---

# {title}

## 原始记录

{raw_text}

## AI 提炼

### 表面症状
{symptom}

### 根因
{root_cause}

### 修复方式
{fix}

### 防错规则
{prevention_rule}

## 建议加入 SOP

{checklist}

## 处理结果

- 状态：{status}
- 文件：{relative_path}
"#,
        yaml = yaml,
        title = input.title,
        raw_text = input.raw_text,
        symptom = input.symptom,
        root_cause = input.root_cause,
        fix = input.fix,
        prevention_rule = input.prevention_rule,
        checklist = checklist,
        status = input.status,
        relative_path = relative_path
    );
    atomic_write(&path, &content)?;
    Ok(path)
}

pub fn write_unprocessed_capture(
    vault_path: &Path,
    capture_id: &str,
    raw_text: &str,
    reason: &str,
    status: CaptureStatus,
) -> Result<PathBuf> {
    let dir = vault_path.join("00_Inbox/Unprocessed");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.md", sanitize_filename(capture_id)));
    let content = format!(
        r#"---
type: unprocessed_capture
id: {capture_id}
status: {status}
---

# Unprocessed Capture {capture_id}

## 原始记录

{raw_text}

## 失败原因

{reason}
"#,
        status = status.as_str()
    );
    atomic_write(&path, &content)?;
    Ok(path)
}

pub fn write_or_patch_sop(vault_path: &Path, input: &SopMarkdownInput) -> Result<SopWriteOutcome> {
    if let Some(existing) = find_sop_by_title(vault_path, &input.title)? {
        let content = fs::read_to_string(&existing)?;
        if content.contains(AUTO_ITEMS_START) && content.contains(AUTO_ITEMS_END) {
            let pit_link = obsidian_link(&input.related_pit_note_stem, &input.related_pit_title);
            let pending_frontmatter = PendingPatchFrontmatter {
                doc_type: Some("pending_patch".to_string()),
                target: Some(path_relative_to(vault_path, &existing)),
                source_pit: Some(input.related_pit_id.clone()),
                status: Some("needs_review".to_string()),
                title: Some(input.title.clone()),
                scenario: Some(input.scenario.clone()),
                risk: Some(input.risk.as_str().to_string()),
                triggers: Some(input.triggers.clone()),
                related_pit: Some(pit_link.clone()),
            };
            let content = ensure_sop_frontmatter(&content, &pending_frontmatter, &existing)?;
            let patched =
                patch_auto_items(&content, &input.checklist_items, &input.related_pit_id)?;
            let patched = append_update_log(&patched, Some(&pit_link));
            atomic_write(&existing, &patched)?;
            return Ok(SopWriteOutcome::Updated { path: existing });
        }

        let patch = write_pending_sop_patch(vault_path, input, &existing)?;
        return Ok(SopWriteOutcome::PendingPatch {
            path: patch,
            target: existing,
        });
    }

    let dir = vault_path.join("02_SOPs").join(&input.category);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.md", sanitize_filename(&input.title)));
    let checklist = input
        .checklist_items
        .iter()
        .map(|item| {
            format!(
                "- [ ] {} <!-- pit2sop:source={} -->",
                item, input.related_pit_id
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let pit_link = obsidian_link(&input.related_pit_note_stem, &input.related_pit_title);
    let frontmatter = SopFrontmatterOut {
        doc_type: "sop",
        id: input.id.clone(),
        version: 1,
        status: "draft".to_string(),
        risk: input.risk.as_str().to_string(),
        scenarios: vec![input.scenario.clone()],
        triggers: input.triggers.clone(),
        related_pits: vec![pit_link.clone()],
        tags: vec!["sop".to_string()],
    };
    let yaml = serde_yaml::to_string(&frontmatter)?;
    let content = format!(
        r#"---
{yaml}---

# {title}

## 适用场景

- {scenario}

## 检查清单

{start}
{checklist}
{end}

## 历史坑点

- {pit_link}

## 更新记录

- {date}：根据 {pit_link} 创建 SOP draft。
"#,
        yaml = yaml,
        title = input.title,
        scenario = input.scenario,
        start = AUTO_ITEMS_START,
        checklist = checklist,
        end = AUTO_ITEMS_END,
        date = Utc::now().format("%Y-%m-%d"),
    );
    atomic_write(&path, &content)?;
    Ok(SopWriteOutcome::Created { path })
}

pub fn load_sop_summaries(vault_path: &Path) -> Result<Vec<SopSummary>> {
    let dir = vault_path.join("02_SOPs");
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sops = Vec::new();
    for entry in WalkDir::new(&dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            continue;
        }
        let content = fs::read_to_string(entry.path())?;
        let frontmatter = parse_frontmatter::<SopFrontmatter>(&content).ok().flatten();
        let has_sop_frontmatter =
            frontmatter.as_ref().and_then(|fm| fm.doc_type.as_deref()) == Some("sop");
        let fm_title = frontmatter.as_ref().and_then(|fm| fm.title.clone());
        let title = first_heading(&content)
            .or(fm_title.clone())
            .unwrap_or_else(|| file_stem(entry.path()));
        let category_signal = entry
            .path()
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        sops.push(SopSummary {
            id: frontmatter
                .as_ref()
                .and_then(|fm| fm.id.clone())
                .unwrap_or_else(|| stable_id("sop", fm_title.as_deref().unwrap_or(&title))),
            title: title.clone(),
            status: frontmatter
                .as_ref()
                .and_then(|fm| fm.status.clone())
                .unwrap_or_else(|| "draft".to_string()),
            risk_level: RiskLevel::from_ai(
                frontmatter
                    .as_ref()
                    .and_then(|fm| fm.risk.as_deref())
                    .unwrap_or(if has_sop_frontmatter { "low" } else { "medium" }),
            ),
            scenarios: frontmatter
                .as_ref()
                .and_then(|fm| fm.scenarios.clone())
                .unwrap_or_else(|| vec![title.clone(), category_signal.clone()]),
            triggers: frontmatter
                .as_ref()
                .and_then(|fm| fm.triggers.clone())
                .unwrap_or_else(|| infer_triggers_from_text(&format!("{title} {content}"))),
            related_pits: frontmatter
                .as_ref()
                .and_then(|fm| fm.related_pits.clone())
                .unwrap_or_default(),
            checklist_items: extract_checklist_items(&content),
            obsidian_path: path_relative_to_path(vault_path, entry.path()),
        });
    }
    Ok(sops)
}

pub fn scan_markdown_documents(vault_path: &Path) -> Result<Vec<MarkdownDocument>> {
    let roots = ["01_Pits", "02_SOPs", "03_Scenes"];
    let mut docs = Vec::new();
    for root in roots {
        let dir = vault_path.join(root);
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            if !entry.file_type().is_file()
                || entry.path().extension().and_then(|ext| ext.to_str()) != Some("md")
            {
                continue;
            }
            let content = fs::read_to_string(entry.path())?;
            let doc_type =
                frontmatter_type(&content).unwrap_or_else(|| fallback_doc_type(root).to_string());
            let title = first_heading(&content).unwrap_or_else(|| {
                entry
                    .path()
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            });
            let path = path_relative_to_path(vault_path, entry.path());
            let path_string = path.to_string_lossy().to_string();
            let doc_id = stable_id(&doc_type, &path_string);
            docs.push(MarkdownDocument {
                doc_id,
                doc_type,
                title,
                path: path_string,
                body: content,
            });
        }
    }
    Ok(docs)
}

pub fn category_for_scenario(scenario: &str) -> String {
    let value = scenario.to_ascii_lowercase();
    if scenario.contains("发布") || value.contains("release") || value.contains("ci") {
        "Release".to_string()
    } else if scenario.contains("客户") || scenario.contains("交付") {
        "Client Delivery".to_string()
    } else if scenario.contains("数据库") || value.contains("migration") || value.contains("code")
    {
        "Development".to_string()
    } else {
        "Personal Workflow".to_string()
    }
}

pub fn normalize_sop_title(raw: &str, scenario: &str) -> String {
    let title = raw.trim();
    if !title.is_empty() && title != "string" {
        if title.starts_with("SOP") {
            title.to_string()
        } else {
            format!("SOP - {}", title)
        }
    } else {
        format!("SOP - {}检查", scenario)
    }
}

pub fn pit_note_stem(input: &PitMarkdownInput) -> String {
    format!(
        "{} {} {}",
        input.created_at.format("%Y-%m-%d"),
        sanitize_filename(&input.title),
        short_id(&input.id)
    )
}

pub fn sop_note_stem(title: &str) -> String {
    sanitize_filename(title)
}

pub fn short_id(id: &str) -> String {
    id.trim_start_matches("pit_")
        .trim_start_matches("cap_")
        .chars()
        .take(8)
        .collect()
}

pub fn stable_id(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    format!("{}_{}", prefix, &hash[..12])
}

pub fn sanitize_filename(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' => ' ',
            _ => ch,
        })
        .collect::<String>();
    let collapsed = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.chars().take(90).collect()
    }
}

fn patch_auto_items(content: &str, items: &[String], source_id: &str) -> Result<String> {
    let start = content
        .find(AUTO_ITEMS_START)
        .ok_or_else(|| anyhow!("missing auto-items start marker"))?;
    let end = content
        .find(AUTO_ITEMS_END)
        .ok_or_else(|| anyhow!("missing auto-items end marker"))?;
    if end < start {
        return Err(anyhow!("auto-items end marker appears before start marker"));
    }

    let before = &content[..start + AUTO_ITEMS_START.len()];
    let block = &content[start + AUTO_ITEMS_START.len()..end];
    let after = &content[end..];
    let mut existing_texts = HashSet::new();
    let mut lines = Vec::new();
    for line in block.lines().map(str::trim).filter(|line| !line.is_empty()) {
        existing_texts.insert(strip_item_text(line));
        lines.push(line.to_string());
    }
    for item in items {
        let normalized = item.trim().to_string();
        if normalized.is_empty() || existing_texts.contains(&normalized) {
            continue;
        }
        lines.push(format!(
            "- [ ] {} <!-- pit2sop:source={} -->",
            normalized, source_id
        ));
        existing_texts.insert(normalized);
    }

    Ok(format!("{}\n{}\n{}", before, lines.join("\n"), after))
}

pub fn write_pending_sop_patch(
    vault_path: &Path,
    input: &SopMarkdownInput,
    target: &Path,
) -> Result<PathBuf> {
    let dir = vault_path.join("99_System/Pending Patches");
    fs::create_dir_all(&dir)?;
    let now = Utc::now();
    let filename = format!(
        "{}-{} {} {}.md",
        now.format("%Y-%m-%d-%H%M%S"),
        now.timestamp_subsec_nanos(),
        sanitize_filename(&input.title),
        short_id(&input.related_pit_id)
    );
    let path = dir.join(filename);
    let checklist = input
        .checklist_items
        .iter()
        .map(|item| format!("- [ ] {}", item))
        .collect::<Vec<_>>()
        .join("\n");
    let target_relative = path_relative_to(vault_path, target);
    let frontmatter = PendingPatchFrontmatterOut {
        doc_type: "pending_patch",
        target: target_relative.clone(),
        source_pit: input.related_pit_id.clone(),
        status: "needs_review".to_string(),
        title: input.title.clone(),
        category: input.category.clone(),
        scenario: input.scenario.clone(),
        risk: input.risk.as_str().to_string(),
        triggers: input.triggers.clone(),
        related_pit: obsidian_link(&input.related_pit_note_stem, &input.related_pit_title),
    };
    let yaml = serde_yaml::to_string(&frontmatter)?;
    let content = format!(
        r#"---
{yaml}---

# Pending Patch - {title}

## 目标 SOP

{target}

## 建议新增检查项

{checklist}
"#,
        yaml = yaml,
        title = input.title,
        target = target_relative,
        checklist = checklist
    );
    atomic_write(&path, &content)?;
    Ok(path)
}

pub fn list_pending_sop_patches(vault_path: &Path) -> Result<Vec<PendingPatchSummary>> {
    let dir = vault_path.join("99_System/Pending Patches");
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut patches = Vec::new();
    for entry in WalkDir::new(&dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            continue;
        }
        let content = fs::read_to_string(entry.path())?;
        let Some(frontmatter) = parse_frontmatter::<PendingPatchFrontmatter>(&content)
            .ok()
            .flatten()
        else {
            continue;
        };
        if frontmatter.doc_type.as_deref() != Some("pending_patch") {
            continue;
        }
        let status = frontmatter
            .status
            .clone()
            .unwrap_or_else(|| "needs_review".to_string());
        if status != "needs_review" {
            continue;
        }
        let title = frontmatter.title.clone().or_else(|| {
            first_heading(&content)
                .and_then(|heading| heading.strip_prefix("Pending Patch - ").map(str::to_string))
        });
        patches.push(PendingPatchSummary {
            path: path_relative_to_path(vault_path, entry.path()),
            target: frontmatter.target.unwrap_or_default(),
            source_pit: frontmatter.source_pit.unwrap_or_default(),
            status,
            title: title.unwrap_or_else(|| {
                entry
                    .path()
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("Untitled pending patch")
                    .to_string()
            }),
        });
    }
    patches.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(patches)
}

pub fn apply_pending_sop_patch(vault_path: &Path, patch_path: &Path) -> Result<PatchActionSummary> {
    let path = resolve_pending_patch_path(vault_path, patch_path)?;
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let frontmatter = parse_pending_patch_frontmatter(&content)?;
    ensure_pending_patch_needs_review(&frontmatter)?;
    let target = frontmatter
        .target
        .as_ref()
        .ok_or_else(|| anyhow!("pending patch is missing target"))?;
    let target_path = resolve_target_inside_vault(vault_path, target)?;
    let checklist_items = extract_pending_checklist_items(&content);
    if checklist_items.is_empty() {
        return Err(anyhow!("pending patch has no checklist items"));
    }
    let source_pit = frontmatter.source_pit.as_deref().unwrap_or("unknown");

    if target_path.exists() {
        let target_content = fs::read_to_string(&target_path)?;
        let target_content = ensure_sop_frontmatter(&target_content, &frontmatter, &target_path)?;
        let patched = if target_content.contains(AUTO_ITEMS_START)
            && target_content.contains(AUTO_ITEMS_END)
        {
            patch_auto_items(&target_content, &checklist_items, source_pit)?
        } else {
            append_auto_items_block(&target_content, &checklist_items, source_pit)
        };
        let patched = append_update_log(&patched, frontmatter.related_pit.as_deref());
        atomic_write(&target_path, &patched)?;
    } else {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = new_sop_from_pending_patch(&frontmatter, &target_path, &checklist_items);
        atomic_write(&target_path, &content)?;
    }

    mark_pit_status_by_id(vault_path, source_pit, "processed")?;
    mark_pending_patch_status(&path, "applied")?;
    let final_path = move_completed_patch(vault_path, &path, "Applied")?;
    Ok(PatchActionSummary {
        path: path_relative_to_path(vault_path, &final_path),
        target_path: Some(path_relative_to_path(vault_path, &target_path)),
        source_pit: frontmatter.source_pit,
        status: "applied".to_string(),
        message: "Pending patch 已应用".to_string(),
    })
}

pub fn reject_pending_sop_patch(
    vault_path: &Path,
    patch_path: &Path,
) -> Result<PatchActionSummary> {
    let path = resolve_pending_patch_path(vault_path, patch_path)?;
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let frontmatter = parse_pending_patch_frontmatter(&content)?;
    ensure_pending_patch_needs_review(&frontmatter)?;
    let target_path = frontmatter
        .target
        .as_deref()
        .map(|target| resolve_target_inside_vault(vault_path, target))
        .transpose()?;
    if let Some(source_pit) = frontmatter.source_pit.as_deref() {
        mark_pit_status_by_id(vault_path, source_pit, "processed")?;
    }
    mark_pending_patch_status(&path, "rejected")?;
    let final_path = move_completed_patch(vault_path, &path, "Rejected")?;
    Ok(PatchActionSummary {
        path: path_relative_to_path(vault_path, &final_path),
        target_path: target_path.map(|target| path_relative_to_path(vault_path, &target)),
        source_pit: frontmatter.source_pit,
        status: "rejected".to_string(),
        message: "Pending patch 已拒绝".to_string(),
    })
}

fn find_sop_by_title(vault_path: &Path, title: &str) -> Result<Option<PathBuf>> {
    let dir = vault_path.join("02_SOPs");
    if !dir.exists() {
        return Ok(None);
    }
    let wanted = sanitize_filename(title);
    for entry in WalkDir::new(dir).into_iter().filter_map(|entry| entry.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let stem = entry
            .path()
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem == wanted || stem == title {
            return Ok(Some(entry.path().to_path_buf()));
        }
    }
    Ok(None)
}

fn atomic_write(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_sibling_path(path);
    fs::write(&tmp, content)?;
    #[cfg(windows)]
    {
        if path.exists() {
            let backup = path.with_extension("pit2sop-bak");
            let _ = fs::remove_file(&backup);
            fs::rename(path, &backup)?;
            if let Err(error) = fs::rename(&tmp, path) {
                let _ = fs::rename(&backup, path);
                return Err(error.into());
            }
            let _ = fs::remove_file(&backup);
            return Ok(());
        }
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn temp_sibling_path(path: &Path) -> PathBuf {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pit2sop")
        .to_string();
    let tmp_name = format!(
        ".{}.{}.tmp",
        filename,
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    path.with_file_name(tmp_name)
}

fn strip_item_text(line: &str) -> String {
    let without_checkbox = line
        .trim_start_matches("- [ ]")
        .trim_start_matches("- [x]")
        .trim();
    without_checkbox
        .split("<!--")
        .next()
        .unwrap_or(without_checkbox)
        .trim()
        .to_string()
}

fn extract_checklist_items(content: &str) -> Vec<String> {
    let Some(start) = content.find(AUTO_ITEMS_START) else {
        return Vec::new();
    };
    let Some(end) = content.find(AUTO_ITEMS_END) else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }

    content[start + AUTO_ITEMS_START.len()..end]
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- [ ]") || line.starts_with("- [x]"))
        .map(strip_item_text)
        .filter(|item| !item.is_empty())
        .collect()
}

fn extract_pending_checklist_items(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- [ ]") || line.starts_with("- [x]"))
        .map(strip_item_text)
        .filter(|item| !item.is_empty())
        .collect()
}

fn ensure_sop_frontmatter(
    content: &str,
    pending: &PendingPatchFrontmatter,
    target_path: &Path,
) -> Result<String> {
    let title = pending
        .title
        .clone()
        .unwrap_or_else(|| file_stem(target_path));
    let mut mapping = if let Some((yaml, _)) = split_frontmatter(content) {
        serde_yaml::from_str::<serde_yaml::Mapping>(yaml)?
    } else {
        serde_yaml::Mapping::new()
    };
    mapping.insert(
        serde_yaml::Value::String("type".to_string()),
        serde_yaml::Value::String("sop".to_string()),
    );
    insert_if_missing(
        &mut mapping,
        "id",
        serde_yaml::Value::String(stable_id("sop", &title)),
    );
    insert_if_missing(
        &mut mapping,
        "version",
        serde_yaml::Value::Number(serde_yaml::Number::from(1)),
    );
    insert_if_missing(
        &mut mapping,
        "status",
        serde_yaml::Value::String("draft".to_string()),
    );
    insert_if_missing(
        &mut mapping,
        "risk",
        serde_yaml::Value::String(pending.risk.clone().unwrap_or_else(|| "medium".to_string())),
    );
    upsert_string_sequence(
        &mut mapping,
        "scenarios",
        pending.scenario.clone().into_iter().collect(),
    );
    upsert_string_sequence(
        &mut mapping,
        "triggers",
        pending.triggers.clone().unwrap_or_default(),
    );
    upsert_string_sequence(
        &mut mapping,
        "related_pits",
        pending.related_pit.clone().into_iter().collect(),
    );
    upsert_string_sequence(&mut mapping, "tags", vec!["sop".to_string()]);

    let yaml = serde_yaml::to_string(&mapping)?;
    let body = split_frontmatter(content)
        .map(|(_, body)| body.trim_start())
        .unwrap_or_else(|| content.trim_start());
    Ok(format!("---\n{}---\n\n{}", yaml, body))
}

fn append_auto_items_block(content: &str, items: &[String], source_id: &str) -> String {
    let checklist = items
        .iter()
        .map(|item| {
            format!(
                "- [ ] {} <!-- pit2sop:source={} -->",
                item.trim(),
                source_id
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n\n## Pit2SOP 自动检查项\n\n{}\n{}\n{}\n",
        content.trim_end(),
        AUTO_ITEMS_START,
        checklist,
        AUTO_ITEMS_END
    )
}

fn append_update_log(content: &str, pit_link: Option<&str>) -> String {
    let Some(pit_link) = pit_link.filter(|value| !value.trim().is_empty()) else {
        return content.to_string();
    };
    if content.contains(&format!("根据 {} 新增检查项", pit_link)) {
        return content.to_string();
    }
    let line = format!(
        "- {}：根据 {} 新增检查项。",
        Utc::now().format("%Y-%m-%d"),
        pit_link
    );
    if content.contains("## Pit2SOP 更新记录") {
        format!("{}\n{}", content.trim_end(), line)
    } else {
        format!(
            "{}\n\n## Pit2SOP 更新记录\n\n{}\n",
            content.trim_end(),
            line
        )
    }
}

fn new_sop_from_pending_patch(
    frontmatter: &PendingPatchFrontmatter,
    target_path: &Path,
    items: &[String],
) -> String {
    let title = frontmatter.title.clone().unwrap_or_else(|| {
        target_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Untitled SOP")
            .to_string()
    });
    let source_pit = frontmatter.source_pit.as_deref().unwrap_or("unknown");
    let checklist = items
        .iter()
        .map(|item| {
            format!(
                "- [ ] {} <!-- pit2sop:source={} -->",
                item.trim(),
                source_pit
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let scenarios = frontmatter
        .scenario
        .clone()
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![value])
        .unwrap_or_default();
    let related_pits = frontmatter
        .related_pit
        .clone()
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![value])
        .unwrap_or_default();
    let sop_frontmatter = SopFrontmatterOut {
        doc_type: "sop",
        id: stable_id("sop", &title),
        version: 1,
        status: "draft".to_string(),
        risk: frontmatter
            .risk
            .clone()
            .unwrap_or_else(|| "medium".to_string()),
        scenarios,
        triggers: frontmatter.triggers.clone().unwrap_or_default(),
        related_pits,
        tags: vec!["sop".to_string()],
    };
    let yaml = serde_yaml::to_string(&sop_frontmatter).unwrap_or_default();
    format!(
        r#"---
{}---

# {}

## 适用场景

{}

## 检查清单

{}
{}
{}

## 更新记录

- {}：根据 pending patch 创建 SOP draft。
"#,
        yaml,
        title,
        frontmatter
            .scenario
            .as_ref()
            .map(|scenario| format!("- {}", scenario))
            .unwrap_or_else(|| "- 待确认".to_string()),
        AUTO_ITEMS_START,
        checklist,
        AUTO_ITEMS_END,
        Utc::now().format("%Y-%m-%d")
    )
}

fn resolve_pending_patch_path(vault_path: &Path, patch_path: &Path) -> Result<PathBuf> {
    let pending_root = vault_path.join("99_System/Pending Patches");
    let path = if patch_path.is_absolute() {
        patch_path.to_path_buf()
    } else {
        let direct = vault_path.join(patch_path);
        if direct.exists() {
            direct
        } else {
            pending_root.join(patch_path)
        }
    };
    let canonical_pending_root = pending_root
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", pending_root.display()))?;
    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    if !canonical_path.starts_with(&canonical_pending_root) {
        return Err(anyhow!(
            "pending patch must be inside vault pending patch directory"
        ));
    }
    Ok(canonical_path)
}

fn resolve_target_inside_vault(vault_path: &Path, target: &str) -> Result<PathBuf> {
    let target_path = PathBuf::from(target);
    if target_path.is_absolute() {
        return Err(anyhow!("pending patch target must be relative to vault"));
    }
    if target_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(anyhow!("pending patch target must stay inside vault"));
    }
    Ok(vault_path.join(target_path))
}

fn move_completed_patch(vault_path: &Path, path: &Path, folder: &str) -> Result<PathBuf> {
    let dir = vault_path.join("99_System/Pending Patches").join(folder);
    fs::create_dir_all(&dir)?;
    let filename = path
        .file_name()
        .ok_or_else(|| anyhow!("pending patch path has no file name"))?;
    let mut destination = dir.join(filename);
    if destination.exists() {
        let stem = destination
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("pending_patch");
        let ext = destination
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("md");
        destination = dir.join(format!(
            "{}-{}.{}",
            stem,
            Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            ext
        ));
    }
    fs::rename(path, &destination)?;
    Ok(destination)
}

fn parse_pending_patch_frontmatter(content: &str) -> Result<PendingPatchFrontmatter> {
    let Some(frontmatter) = parse_frontmatter::<PendingPatchFrontmatter>(content)? else {
        return Err(anyhow!("pending patch is missing frontmatter"));
    };
    if frontmatter.doc_type.as_deref() != Some("pending_patch") {
        return Err(anyhow!("file is not a pending patch"));
    }
    Ok(frontmatter)
}

fn ensure_pending_patch_needs_review(frontmatter: &PendingPatchFrontmatter) -> Result<()> {
    let status = frontmatter.status.as_deref().unwrap_or("needs_review");
    if status != "needs_review" {
        return Err(anyhow!(
            "pending patch status is `{}`, expected `needs_review`",
            status
        ));
    }
    Ok(())
}

fn mark_pending_patch_status(path: &Path, status: &str) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let Some(rest) = content.strip_prefix("---\n") else {
        return Err(anyhow!("pending patch is missing frontmatter"));
    };
    let Some(end) = rest.find("\n---") else {
        return Err(anyhow!("pending patch frontmatter is not closed"));
    };
    let yaml = &rest[..end];
    let body = &rest[end + "\n---".len()..];
    let mut mapping: serde_yaml::Mapping = serde_yaml::from_str(yaml)?;
    mapping.insert(
        serde_yaml::Value::String("status".to_string()),
        serde_yaml::Value::String(status.to_string()),
    );
    let updated_yaml = serde_yaml::to_string(&mapping)?;
    atomic_write(path, &format!("---\n{}---{}", updated_yaml, body))?;
    Ok(())
}

fn mark_pit_status_by_id(vault_path: &Path, pit_id: &str, status: &str) -> Result<bool> {
    if pit_id.trim().is_empty() || pit_id == "unknown" {
        return Ok(false);
    }
    let dir = vault_path.join("01_Pits");
    if !dir.exists() {
        return Ok(false);
    }
    for entry in WalkDir::new(&dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            continue;
        }
        let content = fs::read_to_string(entry.path())?;
        let Some((yaml, body)) = split_frontmatter(&content) else {
            continue;
        };
        let Ok(mut mapping) = serde_yaml::from_str::<serde_yaml::Mapping>(yaml) else {
            continue;
        };
        let id = mapping
            .get(serde_yaml::Value::String("id".to_string()))
            .and_then(|value| value.as_str());
        if id != Some(pit_id) {
            continue;
        }
        mapping.insert(
            serde_yaml::Value::String("status".to_string()),
            serde_yaml::Value::String(status.to_string()),
        );
        let updated_yaml = serde_yaml::to_string(&mapping)?;
        let updated_body = replace_status_line(body, status);
        atomic_write(
            entry.path(),
            &format!("---\n{}---{}", updated_yaml, updated_body),
        )?;
        return Ok(true);
    }
    Ok(false)
}

fn replace_status_line(body: &str, status: &str) -> String {
    body.lines()
        .map(|line| {
            if line.trim_start().starts_with("- 状态：") {
                format!("- 状态：{}", status)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some((&rest[..end], &rest[end + "\n---".len()..]))
}

fn insert_if_missing(mapping: &mut serde_yaml::Mapping, key: &str, value: serde_yaml::Value) {
    let key_value = serde_yaml::Value::String(key.to_string());
    if !mapping.contains_key(&key_value) {
        mapping.insert(key_value, value);
    }
}

fn upsert_string_sequence(mapping: &mut serde_yaml::Mapping, key: &str, values: Vec<String>) {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return;
    }
    let key_value = serde_yaml::Value::String(key.to_string());
    let existing = mapping
        .get(&key_value)
        .and_then(|value| value.as_sequence())
        .cloned()
        .unwrap_or_default();
    for item in existing {
        if let Some(value) = item.as_str() {
            values.push(value.to_string());
        }
    }
    values.sort();
    values.dedup();
    mapping.insert(
        key_value,
        serde_yaml::Value::Sequence(
            values
                .into_iter()
                .map(serde_yaml::Value::String)
                .collect::<Vec<_>>(),
        ),
    );
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Untitled SOP")
        .to_string()
}

fn infer_triggers_from_text(value: &str) -> Vec<String> {
    [
        "上线",
        "发布",
        "release",
        "CI",
        "secret",
        "数据库",
        "迁移",
        "migration",
        "客户",
        "交付",
        "PBS",
        "装反",
        "组装",
    ]
    .into_iter()
    .filter(|trigger| {
        if trigger.is_ascii() {
            value
                .to_ascii_lowercase()
                .contains(&trigger.to_ascii_lowercase())
        } else {
            value.contains(trigger)
        }
    })
    .map(str::to_string)
    .collect()
}

fn path_relative_to(vault_path: &Path, path: &Path) -> String {
    path.strip_prefix(vault_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn path_relative_to_path(vault_path: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(vault_path).unwrap_or(path).to_path_buf()
}

fn parse_frontmatter<T: for<'de> Deserialize<'de>>(content: &str) -> Result<Option<T>> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return Ok(None);
    };
    let Some(end) = rest.find("\n---") else {
        return Ok(None);
    };
    let yaml = &rest[..end];
    let parsed = serde_yaml::from_str(yaml)?;
    Ok(Some(parsed))
}

fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut tags = tags.to_vec();
    if !tags.iter().any(|tag| tag == "pit") {
        tags.push("pit".to_string());
    }
    tags.sort();
    tags.dedup();
    tags
}

fn obsidian_link(note_stem: &str, alias: &str) -> String {
    let alias = alias.replace('|', "/");
    format!("[[{}|{}]]", note_stem, alias)
}

fn frontmatter_type(content: &str) -> Option<String> {
    parse_frontmatter::<GenericFrontmatter>(content)
        .ok()
        .flatten()
        .and_then(|fm| fm.doc_type)
}

fn fallback_doc_type(root: &str) -> &'static str {
    match root {
        "01_Pits" => "pit",
        "02_SOPs" => "sop",
        "03_Scenes" => "scene",
        _ => "markdown",
    }
}

fn first_heading(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        line.strip_prefix("# ")
            .map(|title| title.trim().to_string())
    })
}

#[derive(Debug, Deserialize)]
struct GenericFrontmatter {
    #[serde(rename = "type")]
    doc_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SopFrontmatter {
    #[serde(rename = "type")]
    doc_type: Option<String>,
    id: Option<String>,
    title: Option<String>,
    status: Option<String>,
    risk: Option<String>,
    scenarios: Option<Vec<String>>,
    triggers: Option<Vec<String>>,
    related_pits: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct PitFrontmatter {
    #[serde(rename = "type")]
    doc_type: &'static str,
    id: String,
    created: String,
    source: String,
    status: String,
    scenario: String,
    risk: String,
    recurrence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sop: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SopFrontmatterOut {
    #[serde(rename = "type")]
    doc_type: &'static str,
    id: String,
    version: i64,
    status: String,
    risk: String,
    scenarios: Vec<String>,
    triggers: Vec<String>,
    related_pits: Vec<String>,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PendingPatchFrontmatter {
    #[serde(rename = "type")]
    doc_type: Option<String>,
    target: Option<String>,
    source_pit: Option<String>,
    status: Option<String>,
    title: Option<String>,
    scenario: Option<String>,
    risk: Option<String>,
    triggers: Option<Vec<String>>,
    related_pit: Option<String>,
}

#[derive(Debug, Serialize)]
struct PendingPatchFrontmatterOut {
    #[serde(rename = "type")]
    doc_type: &'static str,
    target: String,
    source_pit: String,
    status: String,
    title: String,
    category: String,
    scenario: String,
    risk: String,
    triggers: Vec<String>,
    related_pit: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn patches_only_marker_block() {
        let content = format!(
            "manual before\n{}\n- [ ] old\n{}\nmanual after",
            AUTO_ITEMS_START, AUTO_ITEMS_END
        );
        let patched = patch_auto_items(&content, &["new".to_string()], "pit_1").unwrap();
        assert!(patched.contains("manual before"));
        assert!(patched.contains("manual after"));
        assert!(patched.contains("- [ ] old"));
        assert!(patched.contains("- [ ] new <!-- pit2sop:source=pit_1 -->"));
    }

    #[test]
    fn existing_sop_without_marker_writes_pending_patch() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target_dir = temp.path().join("02_SOPs/Release");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(
            target_dir.join("SOP - iOS 发布前检查.md"),
            "# SOP - iOS 发布前检查\nmanual",
        )
        .unwrap();
        let outcome = write_or_patch_sop(
            temp.path(),
            &SopMarkdownInput {
                id: "sop_1".into(),
                title: "SOP - iOS 发布前检查".into(),
                category: "Release".into(),
                scenario: "iOS 发布".into(),
                risk: RiskLevel::High,
                triggers: vec!["release".into()],
                related_pit_title: "CI secret 未更新".into(),
                related_pit_id: "pit_1".into(),
                related_pit_note_stem: "2026-05-22 CI secret 未更新 1".into(),
                checklist_items: vec!["检查 CI secret".into()],
            },
        )
        .unwrap();

        assert!(matches!(outcome, SopWriteOutcome::PendingPatch { .. }));
    }

    #[test]
    fn pit_filename_includes_short_id_to_avoid_overwrite() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let created_at = Utc::now();
        let mut input = PitMarkdownInput {
            id: "pit_11111111aaaa".into(),
            title: "CI secret 未更新".into(),
            created_at,
            source: "cli".into(),
            status: "processed".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::High,
            recurrence: RiskLevel::Medium,
            sop_title: None,
            tags: vec!["pit".into()],
            raw_text: "第一次".into(),
            symptom: "失败".into(),
            root_cause: "secret 旧".into(),
            fix: "更新 secret".into(),
            prevention_rule: "检查 secret".into(),
            checklist_items: vec!["检查 secret".into()],
        };
        let first = write_pit(temp.path(), &input).unwrap();
        input.id = "pit_22222222bbbb".into();
        input.raw_text = "第二次".into();
        let second = write_pit(temp.path(), &input).unwrap();

        assert_ne!(first, second);
        assert!(first.exists());
        assert!(second.exists());
        assert!(
            first
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .contains("11111111")
        );
        assert!(
            second
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .contains("22222222")
        );
    }

    #[test]
    fn new_sop_links_to_real_pit_note_stem_with_alias() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let input = SopMarkdownInput {
            id: "sop_1".into(),
            title: "SOP - CI secret 检查".into(),
            category: "Release".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::High,
            triggers: vec!["CI".into()],
            related_pit_title: "CI secret 未更新".into(),
            related_pit_id: "pit_11111111aaaa".into(),
            related_pit_note_stem: "2026-05-22 CI secret 未更新 11111111".into(),
            checklist_items: vec!["检查 CI secret 是否为最新值".into()],
        };

        let outcome = write_or_patch_sop(temp.path(), &input).unwrap();
        let path = match outcome {
            SopWriteOutcome::Created { path } => path,
            _ => panic!("expected created SOP"),
        };
        let content = fs::read_to_string(path).unwrap();

        assert!(content.contains("[[2026-05-22 CI secret 未更新 11111111|CI secret 未更新]]"));
    }

    #[test]
    fn apply_pending_patch_updates_existing_marker_block() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 发布流程检查.md");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(
            &target,
            format!(
                "---\ntype: sop\n---\n# SOP - 发布流程检查\n{}\n- [ ] 旧检查\n{}\n",
                AUTO_ITEMS_START, AUTO_ITEMS_END
            ),
        )
        .unwrap();
        let input = SopMarkdownInput {
            id: "sop_1".into(),
            title: "SOP - 发布流程检查".into(),
            category: "Release".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::High,
            triggers: vec!["发布".into()],
            related_pit_title: "CI secret 未更新".into(),
            related_pit_id: "pit_11111111aaaa".into(),
            related_pit_note_stem: "2026-05-22 CI secret 未更新 11111111".into(),
            checklist_items: vec!["检查 CI secret".into()],
        };
        let patch = write_pending_sop_patch(temp.path(), &input, &target).unwrap();

        let summary = apply_pending_sop_patch(temp.path(), &patch).unwrap();
        let target_content = fs::read_to_string(&target).unwrap();
        let patch_content = fs::read_to_string(temp.path().join(&summary.path)).unwrap();

        assert_eq!(summary.status, "applied");
        assert!(
            summary
                .path
                .starts_with("99_System/Pending Patches/Applied")
        );
        assert!(target_content.contains("- [ ] 旧检查"));
        assert!(target_content.contains("检查 CI secret <!-- pit2sop:source=pit_11111111aaaa -->"));
        assert!(
            target_content.contains("[[2026-05-22 CI secret 未更新 11111111|CI secret 未更新]]")
        );
        assert!(target_content.contains("## Pit2SOP 更新记录"));
        assert!(patch_content.contains("status: applied"));
    }

    #[test]
    fn apply_pending_patch_adds_frontmatter_to_manual_sop_and_loader_falls_back() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 手写发布检查.md");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "# SOP - 手写发布检查\n\nmanual").unwrap();
        let input = SopMarkdownInput {
            id: "sop_1".into(),
            title: "SOP - 手写发布检查".into(),
            category: "Release".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::High,
            triggers: vec!["上线".into()],
            related_pit_title: "CI secret 未更新".into(),
            related_pit_id: "pit_11111111aaaa".into(),
            related_pit_note_stem: "2026-05-22 CI secret 未更新 11111111".into(),
            checklist_items: vec!["检查 CI secret".into()],
        };
        let patch = write_pending_sop_patch(temp.path(), &input, &target).unwrap();

        apply_pending_sop_patch(temp.path(), &patch).unwrap();

        let target_content = fs::read_to_string(&target).unwrap();
        assert!(target_content.starts_with("---\n"));
        assert!(target_content.contains("type: sop"));
        assert!(target_content.contains("triggers:"));
        assert!(target_content.contains("上线"));
        let sops = load_sop_summaries(temp.path()).unwrap();
        assert_eq!(sops.len(), 1);
        assert_eq!(sops[0].title, "SOP - 手写发布检查");
        assert_eq!(sops[0].checklist_items, vec!["检查 CI secret"]);
        assert!(sops[0].triggers.contains(&"上线".to_string()));
    }

    #[test]
    fn load_sop_summaries_includes_manual_sop_without_frontmatter() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - PBS 装反检查.md");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(
            &target,
            format!(
                "# SOP - PBS 装反检查\n\n{}\n- [ ] 检查 PBS 方向\n{}\n",
                AUTO_ITEMS_START, AUTO_ITEMS_END
            ),
        )
        .unwrap();

        let sops = load_sop_summaries(temp.path()).unwrap();

        assert_eq!(sops.len(), 1);
        assert_eq!(sops[0].title, "SOP - PBS 装反检查");
        assert!(sops[0].triggers.contains(&"PBS".to_string()));
        assert_eq!(sops[0].checklist_items, vec!["检查 PBS 方向"]);
    }

    #[test]
    fn load_sop_summaries_ignores_bad_yaml_frontmatter() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 坏 YAML.md");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(
            &target,
            format!(
                "---\ntype: sop\nrisk: \"high\n---\n# SOP - 坏 YAML\n\n{}\n- [ ] 检查坏 YAML 不阻断提醒\n{}\n",
                AUTO_ITEMS_START, AUTO_ITEMS_END
            ),
        )
        .unwrap();

        let sops = load_sop_summaries(temp.path()).unwrap();

        assert_eq!(sops.len(), 1);
        assert_eq!(sops[0].title, "SOP - 坏 YAML");
        assert_eq!(sops[0].checklist_items, vec!["检查坏 YAML 不阻断提醒"]);
    }

    #[test]
    fn scan_markdown_documents_indexes_manual_sop_as_sop() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 手写 SOP.md");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "# SOP - 手写 SOP\n\nmanual").unwrap();

        let docs = scan_markdown_documents(temp.path()).unwrap();
        let doc = docs
            .iter()
            .find(|doc| doc.path.ends_with("SOP - 手写 SOP.md"))
            .unwrap();

        assert_eq!(doc.doc_type, "sop");
    }

    #[test]
    fn apply_pending_patch_creates_missing_sop_and_reject_marks_status() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 新流程检查.md");
        let input = SopMarkdownInput {
            id: "sop_1".into(),
            title: "SOP - 新流程检查".into(),
            category: "Release".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::Medium,
            triggers: vec!["上线".into()],
            related_pit_title: "漏配环境变量".into(),
            related_pit_id: "pit_22222222bbbb".into(),
            related_pit_note_stem: "2026-05-22 漏配环境变量 22222222".into(),
            checklist_items: vec!["确认环境变量".into()],
        };
        let patch = write_pending_sop_patch(temp.path(), &input, &target).unwrap();
        let pending = list_pending_sop_patches(temp.path()).unwrap();
        assert_eq!(pending.len(), 1);

        apply_pending_sop_patch(temp.path(), &patch).unwrap();

        let target_content = fs::read_to_string(&target).unwrap();
        assert!(target_content.contains("type: sop"));
        assert!(target_content.contains("确认环境变量 <!-- pit2sop:source=pit_22222222bbbb -->"));
        assert!(list_pending_sop_patches(temp.path()).unwrap().is_empty());

        let rejected = write_pending_sop_patch(temp.path(), &input, &target).unwrap();
        let summary = reject_pending_sop_patch(temp.path(), &rejected).unwrap();
        let rejected_content = fs::read_to_string(temp.path().join(&summary.path)).unwrap();
        assert_eq!(summary.status, "rejected");
        assert!(
            summary
                .path
                .starts_with("99_System/Pending Patches/Rejected")
        );
        assert!(rejected_content.contains("status: rejected"));
    }

    #[test]
    fn list_pending_sop_patches_skips_bad_yaml() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let dir = temp.path().join("99_System/Pending Patches");
        fs::write(
            dir.join("bad.md"),
            "---\ntype: pending_patch\nstatus: \"needs_review\n---\n# Bad",
        )
        .unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 发布流程检查.md");
        let input = SopMarkdownInput {
            id: "sop_1".into(),
            title: "SOP - 发布流程检查".into(),
            category: "Release".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::Medium,
            triggers: vec!["上线".into()],
            related_pit_title: "漏配环境变量".into(),
            related_pit_id: "pit_22222222bbbb".into(),
            related_pit_note_stem: "2026-05-22 漏配环境变量 22222222".into(),
            checklist_items: vec!["确认环境变量".into()],
        };
        write_pending_sop_patch(temp.path(), &input, &target).unwrap();

        let pending = list_pending_sop_patches(temp.path()).unwrap();

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].source_pit, "pit_22222222bbbb");
    }

    #[test]
    fn apply_and_reject_require_needs_review_status() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 状态检查.md");
        let input = SopMarkdownInput {
            id: "sop_1".into(),
            title: "SOP - 状态检查".into(),
            category: "Release".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::Medium,
            triggers: vec!["上线".into()],
            related_pit_title: "漏配环境变量".into(),
            related_pit_id: "pit_22222222bbbb".into(),
            related_pit_note_stem: "2026-05-22 漏配环境变量 22222222".into(),
            checklist_items: vec!["确认环境变量".into()],
        };
        let applied = write_pending_sop_patch(temp.path(), &input, &target).unwrap();
        mark_pending_patch_status(&applied, "applied").unwrap();
        let rejected = write_pending_sop_patch(temp.path(), &input, &target).unwrap();
        mark_pending_patch_status(&rejected, "rejected").unwrap();

        let apply_error = apply_pending_sop_patch(temp.path(), &applied).unwrap_err();
        let reject_error = reject_pending_sop_patch(temp.path(), &rejected).unwrap_err();

        assert!(apply_error.to_string().contains("expected `needs_review`"));
        assert!(reject_error.to_string().contains("expected `needs_review`"));
    }

    #[test]
    fn pending_patch_file_must_be_inside_pending_directory() {
        let temp = tempdir().unwrap();
        let outside = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 路径检查.md");
        let input = SopMarkdownInput {
            id: "sop_1".into(),
            title: "SOP - 路径检查".into(),
            category: "Release".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::Medium,
            triggers: vec!["上线".into()],
            related_pit_title: "漏配环境变量".into(),
            related_pit_id: "pit_22222222bbbb".into(),
            related_pit_note_stem: "2026-05-22 漏配环境变量 22222222".into(),
            checklist_items: vec!["确认环境变量".into()],
        };
        let patch = write_pending_sop_patch(temp.path(), &input, &target).unwrap();
        let outside_patch = outside.path().join("external.md");
        fs::copy(&patch, &outside_patch).unwrap();

        let error = apply_pending_sop_patch(temp.path(), &outside_patch).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("inside vault pending patch directory")
        );
    }

    #[test]
    fn pending_patch_target_must_stay_inside_vault() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let target = temp.path().join("02_SOPs/Release/SOP - 路径检查.md");
        let input = SopMarkdownInput {
            id: "sop_1".into(),
            title: "SOP - 路径检查".into(),
            category: "Release".into(),
            scenario: "发布流程".into(),
            risk: RiskLevel::Medium,
            triggers: vec!["上线".into()],
            related_pit_title: "漏配环境变量".into(),
            related_pit_id: "pit_22222222bbbb".into(),
            related_pit_note_stem: "2026-05-22 漏配环境变量 22222222".into(),
            checklist_items: vec!["确认环境变量".into()],
        };
        let absolute = write_pending_sop_patch(temp.path(), &input, &target).unwrap();
        let content = fs::read_to_string(&absolute).unwrap();
        fs::write(
            &absolute,
            content.replace(
                "target: 02_SOPs/Release/SOP - 路径检查.md",
                "target: /tmp/outside.md",
            ),
        )
        .unwrap();
        let parent_dir = write_pending_sop_patch(temp.path(), &input, &target).unwrap();
        let content = fs::read_to_string(&parent_dir).unwrap();
        fs::write(
            &parent_dir,
            content.replace(
                "target: 02_SOPs/Release/SOP - 路径检查.md",
                "target: ../outside.md",
            ),
        )
        .unwrap();

        let absolute_error = apply_pending_sop_patch(temp.path(), &absolute).unwrap_err();
        let parent_error = apply_pending_sop_patch(temp.path(), &parent_dir).unwrap_err();

        assert!(
            absolute_error
                .to_string()
                .contains("must be relative to vault")
        );
        assert!(parent_error.to_string().contains("must stay inside vault"));
    }

    #[test]
    fn pit_frontmatter_survives_yaml_special_chars() {
        let temp = tempdir().unwrap();
        init_vault_dirs(temp.path()).unwrap();
        let input = PitMarkdownInput {
            id: "pit_yaml0001".into(),
            title: "标题: \"带引号\"".into(),
            created_at: Utc::now(),
            source: "cli".into(),
            status: "needs_review".into(),
            scenario: "场景: 含冒号\n第二行".into(),
            risk: RiskLevel::Medium,
            recurrence: RiskLevel::Low,
            sop_title: Some("SOP - 特殊字符".into()),
            tags: vec!["a:b".into(), "quote\"tag".into()],
            raw_text: "原始内容".into(),
            symptom: "症状".into(),
            root_cause: "根因".into(),
            fix: "修复".into(),
            prevention_rule: "规则".into(),
            checklist_items: vec!["检查 YAML".into()],
        };
        let path = write_pit(temp.path(), &input).unwrap();
        let content = fs::read_to_string(path).unwrap();
        let yaml = content
            .strip_prefix("---\n")
            .unwrap()
            .split("\n---")
            .next()
            .unwrap();
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(value["scenario"], "场景: 含冒号\n第二行");
        assert_eq!(value["tags"][0], "a:b");
    }

    #[test]
    fn extracts_auto_checklist_items_without_source_comments() {
        let content = format!(
            "{}\n- [ ] 组装PBS前确认拨片朝向大反方向 <!-- pit2sop:source=pit_1 -->\n{}\n",
            AUTO_ITEMS_START, AUTO_ITEMS_END
        );
        assert_eq!(
            extract_checklist_items(&content),
            vec!["组装PBS前确认拨片朝向大反方向"]
        );
    }
}
