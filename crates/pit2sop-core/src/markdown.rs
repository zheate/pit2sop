use crate::models::{RiskLevel, SopSummary};
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Datelike, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
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
    pub checklist_items: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum SopWriteOutcome {
    Created { path: PathBuf },
    Updated { path: PathBuf },
    PendingPatch { path: PathBuf, target: PathBuf },
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
    let filename = format!(
        "{} {}.md",
        input.created_at.format("%Y-%m-%d"),
        sanitize_filename(&input.title)
    );
    let path = dir.join(filename);
    let relative_path = path_relative_to(vault_path, &path);
    let sop_link = input
        .sop_title
        .as_ref()
        .map(|title| format!("\"[[{}]]\"", title))
        .unwrap_or_else(|| "null".to_string());
    let tag_lines = input
        .tags
        .iter()
        .map(|tag| format!("  - {}", tag))
        .collect::<Vec<_>>()
        .join("\n");
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

    let content = format!(
        r#"---
type: pit
id: {id}
created: {created}
source: {source}
status: {status}
scenario: {scenario}
risk: {risk}
recurrence: {recurrence}
sop: {sop_link}
tags:
{tags}
---

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
        id = input.id,
        created = input.created_at.to_rfc3339(),
        source = input.source,
        status = input.status,
        scenario = input.scenario,
        risk = input.risk.as_str(),
        recurrence = input.recurrence.as_str(),
        sop_link = sop_link,
        tags = if tag_lines.is_empty() {
            "  - pit".to_string()
        } else {
            tag_lines
        },
        title = input.title,
        raw_text = input.raw_text,
        symptom = input.symptom,
        root_cause = input.root_cause,
        fix = input.fix,
        prevention_rule = input.prevention_rule,
        checklist = checklist,
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
) -> Result<PathBuf> {
    let dir = vault_path.join("00_Inbox/Unprocessed");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.md", sanitize_filename(capture_id)));
    let content = format!(
        r#"---
type: unprocessed_capture
id: {capture_id}
status: failed
---

# Unprocessed Capture {capture_id}

## 原始记录

{raw_text}

## 失败原因

{reason}
"#
    );
    atomic_write(&path, &content)?;
    Ok(path)
}

pub fn write_or_patch_sop(vault_path: &Path, input: &SopMarkdownInput) -> Result<SopWriteOutcome> {
    if let Some(existing) = find_sop_by_title(vault_path, &input.title)? {
        let content = fs::read_to_string(&existing)?;
        if content.contains(AUTO_ITEMS_START) && content.contains(AUTO_ITEMS_END) {
            let patched =
                patch_auto_items(&content, &input.checklist_items, &input.related_pit_id)?;
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
    let trigger_lines = yaml_list(&input.triggers);
    let scenario_lines = yaml_list(std::slice::from_ref(&input.scenario));
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
    let content = format!(
        r#"---
type: sop
id: {id}
version: 1
status: draft
risk: {risk}
scenarios:
{scenarios}
triggers:
{triggers}
related_pits:
  - "[[{related_pit_title}]]"
tags:
  - sop
---

# {title}

## 适用场景

- {scenario}

## 检查清单

{start}
{checklist}
{end}

## 历史坑点

- [[{related_pit_title}]]

## 更新记录

- {date}：根据 [[{related_pit_title}]] 创建 SOP draft。
"#,
        id = input.id,
        risk = input.risk.as_str(),
        scenarios = scenario_lines,
        triggers = trigger_lines,
        related_pit_title = input.related_pit_title,
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
        let Some(frontmatter) = parse_frontmatter::<SopFrontmatter>(&content)? else {
            continue;
        };
        if frontmatter.doc_type.as_deref() != Some("sop") {
            continue;
        }
        let fm_title = frontmatter.title.clone();
        let title = first_heading(&content)
            .or(fm_title.clone())
            .unwrap_or_else(|| "Untitled SOP".to_string());
        sops.push(SopSummary {
            id: frontmatter
                .id
                .unwrap_or_else(|| stable_id("sop", fm_title.as_deref().unwrap_or(&title))),
            title,
            status: frontmatter.status.unwrap_or_else(|| "draft".to_string()),
            risk_level: RiskLevel::from_ai(frontmatter.risk.as_deref().unwrap_or("low")),
            scenarios: frontmatter.scenarios.unwrap_or_default(),
            triggers: frontmatter.triggers.unwrap_or_default(),
            related_pits: frontmatter.related_pits.unwrap_or_default(),
            checklist_items: extract_checklist_items(&content),
            obsidian_path: path_relative_to_path(vault_path, entry.path()),
        });
    }
    Ok(sops)
}

pub fn scan_markdown_documents(
    vault_path: &Path,
) -> Result<Vec<(String, String, String, String, String)>> {
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
            let doc_type = frontmatter_type(&content).unwrap_or_else(|| root.to_string());
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
            docs.push((doc_id, doc_type, title, path_string, content));
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
    let filename = format!(
        "{} {}.md",
        Utc::now().format("%Y-%m-%d-%H%M%S"),
        sanitize_filename(&input.title)
    );
    let path = dir.join(filename);
    let checklist = input
        .checklist_items
        .iter()
        .map(|item| format!("- [ ] {}", item))
        .collect::<Vec<_>>()
        .join("\n");
    let content = format!(
        r#"---
type: pending_patch
target: "{}"
source_pit: {}
status: needs_review
---

# Pending Patch - {}

## 目标 SOP

{}

## 建议新增检查项

{}
"#,
        path_relative_to(vault_path, target),
        input.related_pit_id,
        input.title,
        path_relative_to(vault_path, target),
        checklist
    );
    atomic_write(&path, &content)?;
    Ok(path)
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
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn yaml_list(values: &[String]) -> String {
    if values.is_empty() {
        return "  - 未分类".to_string();
    }
    values
        .iter()
        .map(|value| format!("  - {}", value))
        .collect::<Vec<_>>()
        .join("\n")
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

fn frontmatter_type(content: &str) -> Option<String> {
    parse_frontmatter::<GenericFrontmatter>(content)
        .ok()
        .flatten()
        .and_then(|fm| fm.doc_type)
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
                checklist_items: vec!["检查 CI secret".into()],
            },
        )
        .unwrap();

        assert!(matches!(outcome, SopWriteOutcome::PendingPatch { .. }));
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
