use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSourceType {
    MobileVoice,
    MobileText,
    MobileShare,
    DesktopVoice,
    DesktopText,
    Cli,
    Git,
    File,
    Browser,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStatus {
    Created,
    Queued,
    Sending,
    Delivered,
    Processing,
    Processed,
    Failed,
    NeedsReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn from_ai(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "high" | "高" | "高风险" => Self::High,
            "medium" | "中" | "中风险" => Self::Medium,
            _ => Self::Low,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureEvent {
    pub id: String,
    pub source_device: String,
    pub source_type: CaptureSourceType,
    pub created_at: DateTime<Utc>,
    pub timezone: String,
    pub raw_text: Option<String>,
    pub status: CaptureStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pit {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub source_capture_id: String,
    pub scenario: String,
    pub symptom: String,
    pub root_cause: String,
    pub fix: String,
    pub prevention_rule: String,
    pub risk_level: RiskLevel,
    pub recurrence_probability: RiskLevel,
    pub sop_title: Option<String>,
    pub tags: Vec<String>,
    pub obsidian_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SopSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub risk_level: RiskLevel,
    pub scenarios: Vec<String>,
    pub triggers: Vec<String>,
    pub related_pits: Vec<String>,
    pub checklist_items: Vec<String>,
    pub obsidian_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub doc_type: String,
    pub title: String,
    pub path: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoingMatch {
    pub score: f32,
    pub title: String,
    pub path: String,
    pub risk: RiskLevel,
    pub scenarios: Vec<String>,
    pub checklist_items: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingSummary {
    pub capture_id: String,
    pub status: CaptureStatus,
    pub pit_path: Option<PathBuf>,
    pub sop_path: Option<PathBuf>,
    pub pending_patch_path: Option<PathBuf>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatus {
    pub vault_path: PathBuf,
    pub db_path: PathBuf,
    pub ai_provider: String,
    pub ai_model: String,
    pub secrets_configured: bool,
    pub indexed_docs: usize,
    pub pit_files: usize,
    pub sop_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPitResponse {
    pub classification: String,
    pub confidence: f32,
    pub pit: Option<AiPitFields>,
    pub sop_action: AiSopAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPitFields {
    pub title: String,
    pub scenario: String,
    pub symptom: String,
    pub root_cause: String,
    pub fix: String,
    pub prevention_rule: String,
    pub risk_level: String,
    pub recurrence_probability: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub trigger_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSopAction {
    #[serde(rename = "type")]
    pub action_type: String,
    pub sop_title: String,
    #[serde(default)]
    pub checklist_items: Vec<String>,
    pub reason: String,
}
