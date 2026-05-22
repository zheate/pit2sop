use crate::ai::{build_ai_provider, validate_ai_pit_response};
use crate::config::{AppConfig, Secrets};
use crate::db::{Database, PitRecord, SopRecord};
use crate::markdown::{
    PitMarkdownInput, SopMarkdownInput, SopWriteOutcome, apply_pending_sop_patch,
    category_for_scenario, init_vault_dirs, list_pending_sop_patches, load_sop_summaries,
    normalize_sop_title, pit_note_stem, reject_pending_sop_patch, sanitize_filename,
    scan_markdown_documents, stable_id, write_or_patch_sop, write_pending_sop_patch, write_pit,
    write_unprocessed_capture,
};
use crate::matcher::match_doing;
use crate::models::{
    AppStatus, CaptureStatus, DoingMatch, PatchActionSummary, PendingPatchSummary,
    ProcessingSummary, RiskLevel, SearchResult, SopSummary,
};
use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct Pit2Sop {
    pub config: AppConfig,
    pub secrets: Secrets,
}

impl Pit2Sop {
    pub fn load() -> Result<Self> {
        Ok(Self {
            config: AppConfig::load_or_default()?,
            secrets: Secrets::load_or_default()?,
        })
    }

    pub fn with_config(config: AppConfig, secrets: Secrets) -> Self {
        Self { config, secrets }
    }

    pub fn init(vault_path: Option<PathBuf>) -> Result<AppConfig> {
        let mut config = AppConfig::load_or_default()?;
        if let Some(path) = vault_path {
            config.vault_path = path;
        }
        init_vault_dirs(&config.vault_path)?;
        config.save()?;
        Secrets::default().save_if_missing()?;
        Database::open(&config.db_path())?;
        Ok(config)
    }

    pub fn process_pit(&self, raw_text: &str) -> Result<ProcessingSummary> {
        let db = self.open_db()?;
        let capture_id = generate_capture_id(raw_text);
        let now = Utc::now();
        db.upsert_capture(
            &capture_id,
            "cli",
            CaptureStatus::Processing,
            raw_text,
            &now.to_rfc3339(),
        )?;

        let existing_sops = load_sop_summaries(&self.config.vault_path)?;
        let provider = match build_ai_provider(&self.config, &self.secrets) {
            Ok(provider) => provider,
            Err(error) => {
                let path = write_unprocessed_capture(
                    &self.config.vault_path,
                    &capture_id,
                    raw_text,
                    &error.to_string(),
                )?;
                db.mark_capture(
                    &capture_id,
                    CaptureStatus::Failed,
                    Some(&path.to_string_lossy()),
                    Some(&error.to_string()),
                )?;
                return Err(error);
            }
        };

        let ai = match provider.extract_pit(raw_text, &existing_sops) {
            Ok(response) => response,
            Err(error) => {
                let path = write_unprocessed_capture(
                    &self.config.vault_path,
                    &capture_id,
                    raw_text,
                    &error.to_string(),
                )?;
                db.mark_capture(
                    &capture_id,
                    CaptureStatus::Failed,
                    Some(&path.to_string_lossy()),
                    Some(&error.to_string()),
                )?;
                return Err(error);
            }
        };
        if let Err(error) = validate_ai_pit_response(&ai) {
            let path = write_unprocessed_capture(
                &self.config.vault_path,
                &capture_id,
                raw_text,
                &error.to_string(),
            )?;
            db.mark_capture(
                &capture_id,
                CaptureStatus::Failed,
                Some(&path.to_string_lossy()),
                Some(&error.to_string()),
            )?;
            return Err(error);
        }

        if ai.classification != "pit" {
            let reason = format!(
                "AI classified input as `{}` instead of `pit`",
                ai.classification
            );
            let path =
                write_unprocessed_capture(&self.config.vault_path, &capture_id, raw_text, &reason)?;
            db.mark_capture(
                &capture_id,
                CaptureStatus::NeedsReview,
                Some(&path.to_string_lossy()),
                Some("input was not classified as pit"),
            )?;
            return Ok(ProcessingSummary {
                capture_id,
                status: CaptureStatus::NeedsReview,
                pit_path: None,
                sop_path: None,
                pending_patch_path: Some(path),
                message: format!("输入被分类为 `{}`，未自动写入 Pit", ai.classification),
            });
        }

        let Some(pit_fields) = ai.pit else {
            let path = write_unprocessed_capture(
                &self.config.vault_path,
                &capture_id,
                raw_text,
                "AI did not return pit fields",
            )?;
            db.mark_capture(
                &capture_id,
                CaptureStatus::NeedsReview,
                Some(&path.to_string_lossy()),
                Some("AI did not return pit fields"),
            )?;
            return Ok(ProcessingSummary {
                capture_id,
                status: CaptureStatus::NeedsReview,
                pit_path: None,
                sop_path: None,
                pending_patch_path: Some(path),
                message: "AI 没有返回 Pit 字段，已进入待确认".to_string(),
            });
        };

        let pit_id = stable_id("pit", &capture_id);
        let risk = RiskLevel::from_ai(&pit_fields.risk_level);
        let recurrence = RiskLevel::from_ai(&pit_fields.recurrence_probability);
        let should_write_sop = matches!(
            ai.sop_action.action_type.as_str(),
            "update_existing" | "create_new"
        );
        let can_auto_write_sop = should_write_sop && ai.confidence >= 0.65;
        let mut status = if can_auto_write_sop {
            "processed".to_string()
        } else {
            "needs_review".to_string()
        };
        let checklist_items = if ai.sop_action.checklist_items.is_empty() {
            vec![pit_fields.prevention_rule.clone()]
        } else {
            ai.sop_action.checklist_items.clone()
        };
        let should_write_pending_patch = matches!(
            ai.sop_action.action_type.as_str(),
            "update_existing" | "create_new" | "needs_review"
        ) && !checklist_items.is_empty();
        let sop_title = (should_write_sop || should_write_pending_patch).then(|| {
            guarded_sop_title(
                &ai.sop_action.sop_title,
                &pit_fields.scenario,
                &pit_fields.trigger_keywords,
                &existing_sops,
            )
        });

        let pit_input = PitMarkdownInput {
            id: pit_id.clone(),
            title: pit_fields.title.clone(),
            created_at: now,
            source: "cli".to_string(),
            status: status.clone(),
            scenario: pit_fields.scenario.clone(),
            risk: risk.clone(),
            recurrence: recurrence.clone(),
            sop_title: sop_title.clone(),
            tags: merge_tags(pit_fields.tags.clone(), pit_fields.trigger_keywords.clone()),
            raw_text: raw_text.to_string(),
            symptom: pit_fields.symptom.clone(),
            root_cause: pit_fields.root_cause.clone(),
            fix: pit_fields.fix.clone(),
            prevention_rule: pit_fields.prevention_rule.clone(),
            checklist_items: checklist_items.clone(),
        };
        let related_pit_note_stem = pit_note_stem(&pit_input);
        let pit_path = write_pit(&self.config.vault_path, &pit_input)?;
        let (sop_path, pending_patch_path) = if let Some(sop_title) = &sop_title {
            let sop_input = SopMarkdownInput {
                id: stable_id("sop", sop_title),
                title: sop_title.clone(),
                category: category_for_scenario(&pit_fields.scenario),
                scenario: pit_fields.scenario.clone(),
                risk: risk.clone(),
                triggers: pit_fields.trigger_keywords.clone(),
                related_pit_title: pit_fields.title.clone(),
                related_pit_id: pit_id.clone(),
                related_pit_note_stem,
                checklist_items: checklist_items.clone(),
            };
            let sop_outcome = if can_auto_write_sop {
                write_or_patch_sop(&self.config.vault_path, &sop_input)?
            } else {
                let target = self
                    .config
                    .vault_path
                    .join("02_SOPs")
                    .join(&sop_input.category)
                    .join(format!("{}.md", sanitize_filename(&sop_input.title)));
                let pending =
                    write_pending_sop_patch(&self.config.vault_path, &sop_input, &target)?;
                SopWriteOutcome::PendingPatch {
                    path: pending,
                    target,
                }
            };

            match &sop_outcome {
                SopWriteOutcome::Created { path } | SopWriteOutcome::Updated { path } => {
                    (Some(path.clone()), None)
                }
                SopWriteOutcome::PendingPatch { path, target: _ } => (None, Some(path.clone())),
            }
        } else {
            (None, None)
        };
        if pending_patch_path.is_some() && status == "processed" {
            status = "needs_review".to_string();
            let mut reviewed_pit_input = pit_input.clone();
            reviewed_pit_input.status = status.clone();
            write_pit(&self.config.vault_path, &reviewed_pit_input)?;
        }
        let pit_relative_path = relative(&self.config.vault_path, &pit_path);
        let now_string = now.to_rfc3339();
        db.upsert_pit(PitRecord {
            id: &pit_id,
            capture_id: &capture_id,
            title: &pit_fields.title,
            scenario: &pit_fields.scenario,
            risk: risk.as_str(),
            recurrence: recurrence.as_str(),
            sop_title: sop_title.as_deref(),
            file_path: &pit_relative_path,
            created_at: &now_string,
        })?;
        if let (Some(path), Some(sop_title)) = (&sop_path, &sop_title) {
            let sop_id = stable_id("sop", sop_title);
            let sop_relative_path = relative(&self.config.vault_path, path);
            db.upsert_sop(SopRecord {
                id: &sop_id,
                title: sop_title,
                status: "draft",
                risk: risk.as_str(),
                version: 1,
                file_path: &sop_relative_path,
                updated_at: &now_string,
            })?;
        }
        db.mark_capture(
            &capture_id,
            if status == "processed" {
                CaptureStatus::Processed
            } else {
                CaptureStatus::NeedsReview
            },
            Some(&relative(&self.config.vault_path, &pit_path)),
            None,
        )?;
        self.reindex()?;

        Ok(ProcessingSummary {
            capture_id,
            status: if status == "processed" {
                CaptureStatus::Processed
            } else {
                CaptureStatus::NeedsReview
            },
            pit_path: Some(pit_path),
            sop_path,
            pending_patch_path,
            message: "Pit 已处理".to_string(),
        })
    }

    pub fn doing(&self, text: &str) -> Result<Vec<DoingMatch>> {
        let sops = load_sop_summaries(&self.config.vault_path)?;
        Ok(match_doing(text, &sops))
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let db = self.open_db()?;
        db.search(query, 20)
    }

    pub fn reindex(&self) -> Result<usize> {
        let db = self.open_db()?;
        db.clear_search_index()?;
        let docs = scan_markdown_documents(&self.config.vault_path)?;
        let count = docs.len();
        for doc in docs {
            db.index_document(&doc.doc_id, &doc.doc_type, &doc.title, &doc.path, &doc.body)?;
        }
        Ok(count)
    }

    pub fn load_sops(&self) -> Result<Vec<SopSummary>> {
        load_sop_summaries(&self.config.vault_path)
    }

    pub fn pending_patches(&self) -> Result<Vec<PendingPatchSummary>> {
        list_pending_sop_patches(&self.config.vault_path)
    }

    pub fn apply_pending_patch(&self, path: &Path) -> Result<PatchActionSummary> {
        let summary = apply_pending_sop_patch(&self.config.vault_path, path)?;
        self.reindex()?;
        Ok(summary)
    }

    pub fn reject_pending_patch(&self, path: &Path) -> Result<PatchActionSummary> {
        reject_pending_sop_patch(&self.config.vault_path, path)
    }

    pub fn status(&self) -> Result<AppStatus> {
        let db = self.open_db()?;
        let docs = scan_markdown_documents(&self.config.vault_path)?;
        Ok(AppStatus {
            vault_path: self.config.vault_path.clone(),
            db_path: self.config.db_path(),
            ai_provider: self.config.ai.provider.clone(),
            ai_model: self.config.ai.model.clone(),
            secrets_configured: self.secrets.has_deepseek_key(),
            indexed_docs: db.count_indexed_docs()?,
            pit_files: docs.iter().filter(|doc| doc.doc_type == "pit").count(),
            sop_files: docs.iter().filter(|doc| doc.doc_type == "sop").count(),
        })
    }

    fn open_db(&self) -> Result<Database> {
        Database::open(&self.config.db_path()).with_context(|| {
            format!(
                "failed to open database for {}",
                self.config.vault_path.display()
            )
        })
    }
}

fn generate_capture_id(raw_text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"cli:");
    hasher.update(Utc::now().to_rfc3339().as_bytes());
    hasher.update(raw_text.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    format!("cap_{}", &hash[..12])
}

fn merge_tags(mut left: Vec<String>, right: Vec<String>) -> Vec<String> {
    left.push("pit".to_string());
    for item in right {
        if !left.contains(&item) {
            left.push(item);
        }
    }
    left.sort();
    left.dedup();
    left
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn guarded_sop_title(
    ai_sop_title: &str,
    scenario: &str,
    trigger_keywords: &[String],
    existing_sops: &[SopSummary],
) -> String {
    let normalized = normalize_sop_title(ai_sop_title, scenario);
    let Some(existing) = existing_sops.iter().find(|sop| sop.title == normalized) else {
        return normalized;
    };

    if has_sop_context_overlap(existing, scenario, trigger_keywords) {
        normalized
    } else {
        normalize_sop_title("", scenario)
    }
}

fn has_sop_context_overlap(sop: &SopSummary, scenario: &str, trigger_keywords: &[String]) -> bool {
    let scenario = scenario.trim();
    if sop
        .scenarios
        .iter()
        .any(|existing| existing.trim() == scenario)
    {
        return true;
    }

    trigger_keywords.iter().any(|keyword| {
        sop.triggers
            .iter()
            .any(|existing| same_keyword(existing, keyword))
    })
}

fn same_keyword(left: &str, right: &str) -> bool {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() || right.is_empty() {
        return false;
    }

    if left.is_ascii() && right.is_ascii() {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AiConfig;
    use rusqlite::Connection;
    use std::fs;
    use tempfile::tempdir;

    fn test_config(temp: &tempfile::TempDir) -> AppConfig {
        AppConfig {
            vault_path: temp.path().join("vault"),
            language: "zh-CN".into(),
            db_path_override: Some(temp.path().join("db.sqlite")),
            ai: AiConfig {
                provider: "heuristic".into(),
                model: "heuristic".into(),
                base_url: "local".into(),
            },
        }
    }

    #[test]
    fn init_creates_vault_dirs_and_reindex_starts_empty() {
        let temp = tempdir().unwrap();
        let config = test_config(&temp);
        init_vault_dirs(&config.vault_path).unwrap();
        let app = Pit2Sop::with_config(config, Secrets::default());
        let count = app.reindex().unwrap();
        assert_eq!(count, 0);
        assert!(app.config.vault_path.join("02_SOPs/Release").exists());
    }

    #[test]
    fn heuristic_pit_creates_markdown_and_search_index() {
        let temp = tempdir().unwrap();
        let config = test_config(&temp);
        init_vault_dirs(&config.vault_path).unwrap();
        let app = Pit2Sop::with_config(config, Secrets::default());
        let summary = app
            .process_pit("今天上线漏了 CI secret，导致 production 请求失败")
            .unwrap();
        assert_eq!(summary.status, CaptureStatus::Processed);
        assert!(summary.pit_path.unwrap().exists());
        let results = app.search("secret").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn repeated_raw_text_creates_distinct_pit_files() {
        let temp = tempdir().unwrap();
        let config = test_config(&temp);
        init_vault_dirs(&config.vault_path).unwrap();
        let app = Pit2Sop::with_config(config, Secrets::default());

        let first = app
            .process_pit("今天上线漏了 CI secret，导致 production 请求失败")
            .unwrap();
        let second = app
            .process_pit("今天上线漏了 CI secret，导致 production 请求失败")
            .unwrap();

        let first_path = first.pit_path.unwrap();
        let second_path = second.pit_path.unwrap();
        assert_ne!(first.capture_id, second.capture_id);
        assert_ne!(first_path, second_path);
        assert!(first_path.exists());
        assert!(second_path.exists());
    }

    #[test]
    fn note_classification_goes_to_unprocessed_without_pit() {
        let temp = tempdir().unwrap();
        let config = test_config(&temp);
        init_vault_dirs(&config.vault_path).unwrap();
        let app = Pit2Sop::with_config(config, Secrets::default());

        let summary = app.process_pit("普通笔记：今天只是记录想法").unwrap();

        assert_eq!(summary.status, CaptureStatus::NeedsReview);
        assert!(summary.pit_path.is_none());
        assert!(summary.pending_patch_path.unwrap().exists());
        assert!(
            fs::read_dir(app.config.vault_path.join("01_Pits"))
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn sop_action_none_writes_pit_but_no_sop() {
        let temp = tempdir().unwrap();
        let config = test_config(&temp);
        init_vault_dirs(&config.vault_path).unwrap();
        let app = Pit2Sop::with_config(config, Secrets::default());

        let summary = app
            .process_pit("今天上线出现问题，但这次不需要 SOP")
            .unwrap();

        assert_eq!(summary.status, CaptureStatus::NeedsReview);
        assert!(summary.pit_path.unwrap().exists());
        assert!(summary.sop_path.is_none());
        assert!(summary.pending_patch_path.is_none());
    }

    #[test]
    fn invalid_ai_response_marks_capture_failed() {
        let temp = tempdir().unwrap();
        let config = test_config(&temp);
        init_vault_dirs(&config.vault_path).unwrap();
        let app = Pit2Sop::with_config(config, Secrets::default());

        let error = app.process_pit("无效 AI 响应").unwrap_err();

        assert!(error.to_string().contains("AI confidence"));
        let conn = Connection::open(app.config.db_path()).unwrap();
        let (status, error): (String, String) = conn
            .query_row(
                "SELECT status, error FROM capture_events LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "failed");
        assert!(error.contains("AI confidence"));
        assert!(
            fs::read_dir(app.config.vault_path.join("00_Inbox/Unprocessed"))
                .unwrap()
                .next()
                .is_some()
        );
    }

    #[test]
    fn needs_review_writes_pending_patch_without_upserting_sop() {
        let temp = tempdir().unwrap();
        let config = test_config(&temp);
        init_vault_dirs(&config.vault_path).unwrap();
        let app = Pit2Sop::with_config(config, Secrets::default());

        let summary = app
            .process_pit("今天上线漏了 CI secret，需要人工确认 SOP")
            .unwrap();

        assert_eq!(summary.status, CaptureStatus::NeedsReview);
        assert!(summary.sop_path.is_none());
        assert!(summary.pending_patch_path.unwrap().exists());
        let conn = Connection::open(app.config.db_path()).unwrap();
        let sop_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sops", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sop_count, 0);
    }

    #[test]
    fn reindex_searches_chinese_content_with_cache_table() {
        let temp = tempdir().unwrap();
        let config = test_config(&temp);
        init_vault_dirs(&config.vault_path).unwrap();
        let app = Pit2Sop::with_config(config, Secrets::default());
        fs::create_dir_all(app.config.vault_path.join("01_Pits/2026")).unwrap();
        fs::write(
            app.config
                .vault_path
                .join("01_Pits/2026/2026-05-22 PBS 装反 pit_1.md"),
            "---\ntype: pit\n---\n# PBS 装反\n组装过程中 PBS 装反，拨片方向错误。",
        )
        .unwrap();

        app.reindex().unwrap();
        let results = app.search("装反").unwrap();

        assert!(results.iter().any(|item| item.title.contains("PBS 装反")));
    }

    #[test]
    fn guarded_sop_title_rejects_unrelated_existing_sop() {
        let existing = vec![SopSummary {
            id: "sop_release".into(),
            title: "SOP - 上线前配置项检查清单".into(),
            status: "draft".into(),
            risk_level: RiskLevel::High,
            scenarios: vec!["线上部署与发布流程".into()],
            triggers: vec!["上线".into(), "CI secret".into(), "production".into()],
            related_pits: vec![],
            checklist_items: vec!["检查 CI secret".into()],
            obsidian_path: PathBuf::from("02_SOPs/Release/SOP - 上线前配置项检查清单.md"),
        }];

        let title = guarded_sop_title(
            "SOP - 上线前配置项检查清单",
            "数据库变更与线上数据兼容性",
            &["数据库迁移".into(), "默认值".into(), "migration".into()],
            &existing,
        );

        assert_eq!(title, "SOP - 数据库变更与线上数据兼容性检查");
    }
}
