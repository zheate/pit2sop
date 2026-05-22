use crate::config::{AppConfig, Secrets};
use crate::models::{AiPitFields, AiPitResponse, AiSopAction, SopSummary};
use anyhow::{Context, Result, anyhow};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

pub trait AiProvider {
    fn extract_pit(&self, raw_text: &str, existing_sops: &[SopSummary]) -> Result<AiPitResponse>;
}

pub fn build_ai_provider(config: &AppConfig, secrets: &Secrets) -> Result<Box<dyn AiProvider>> {
    match config.ai.provider.as_str() {
        "deepseek" => {
            let api_key = secrets
                .deepseek_api_key
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .ok_or_else(|| {
                    anyhow!(
                        "missing DeepSeek API key. Set deepseek_api_key in {}",
                        crate::config::secrets_path().display()
                    )
                })?;
            Ok(Box::new(DeepSeekProvider::new(
                config.ai.base_url.clone(),
                config.ai.model.clone(),
                api_key,
            )?))
        }
        "heuristic" => Ok(Box::new(HeuristicProvider)),
        other => Err(anyhow!("unsupported AI provider: {}", other)),
    }
}

pub fn validate_ai_pit_response(response: &AiPitResponse) -> Result<()> {
    if !(0.0..=1.0).contains(&response.confidence) {
        return Err(anyhow!("AI confidence must be between 0 and 1"));
    }
    if !matches!(
        response.classification.as_str(),
        "pit" | "doing" | "note" | "log" | "sop_request"
    ) {
        return Err(anyhow!(
            "unsupported AI classification `{}`",
            response.classification
        ));
    }
    if response.sop_action.action_type.trim().is_empty() {
        return Err(anyhow!("AI sop_action.type is empty"));
    }
    if !matches!(
        response.sop_action.action_type.as_str(),
        "update_existing" | "create_new" | "needs_review" | "none"
    ) {
        return Err(anyhow!(
            "unsupported SOP action `{}`",
            response.sop_action.action_type
        ));
    }

    if response.classification == "pit" {
        let pit = response
            .pit
            .as_ref()
            .ok_or_else(|| anyhow!("AI classified input as pit but returned no pit object"))?;
        for (name, value) in [
            ("title", pit.title.as_str()),
            ("scenario", pit.scenario.as_str()),
            ("symptom", pit.symptom.as_str()),
            ("root_cause", pit.root_cause.as_str()),
            ("fix", pit.fix.as_str()),
            ("prevention_rule", pit.prevention_rule.as_str()),
        ] {
            if value.trim().is_empty() || value.trim() == "string" {
                return Err(anyhow!("AI pit field `{}` is empty or placeholder", name));
            }
        }
    }

    Ok(())
}

struct DeepSeekProvider {
    base_url: String,
    model: String,
    api_key: String,
    client: Client,
}

impl DeepSeekProvider {
    fn new(base_url: String, model: String, api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .context("failed to build DeepSeek HTTP client")?;
        Ok(Self {
            base_url,
            model,
            api_key,
            client,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

impl AiProvider for DeepSeekProvider {
    fn extract_pit(&self, raw_text: &str, existing_sops: &[SopSummary]) -> Result<AiPitResponse> {
        let sops = existing_sops
            .iter()
            .map(|sop| {
                json!({
                    "id": sop.id,
                    "title": sop.title,
                    "scenarios": sop.scenarios,
                    "triggers": sop.triggers,
                    "risk": sop.risk_level.as_str(),
                })
            })
            .collect::<Vec<_>>();

        let prompt = format!(
            r#"你是 Pit2SOP 的结构化处理器。只输出严格 JSON，不要 Markdown。

任务：
1. 判断输入是否是一次踩坑记录。
2. 如果是 pit，提取症状、根因、修复方式、防错规则。
3. 选择更新已有 SOP、创建新 SOP、needs_review 或 none。
4. checklist_items 必须是可执行动作句，不能是空泛提醒。

已有 SOP 候选：
{}

必须输出这个 JSON 结构：
{{
  "classification": "pit|doing|note|log|sop_request",
  "confidence": 0.0,
  "pit": {{
    "title": "string",
    "scenario": "string",
    "symptom": "string",
    "root_cause": "string",
    "fix": "string",
    "prevention_rule": "string",
    "risk_level": "low|medium|high",
    "recurrence_probability": "low|medium|high",
    "tags": ["string"],
    "trigger_keywords": ["string"]
  }},
  "sop_action": {{
    "type": "update_existing|create_new|needs_review|none",
    "sop_title": "string",
    "checklist_items": ["string"],
    "reason": "string"
  }}
}}

输入：
{}"#,
            serde_json::to_string_pretty(&sops)?,
            raw_text
        );

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "你只返回可被 JSON parser 解析的对象。".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            temperature: 0.0,
            max_tokens: 2048,
            response_format: json!({ "type": "json_object" }),
            thinking: json!({ "type": "disabled" }),
        };

        let mut last_empty = false;
        for _ in 0..2 {
            let response = self
                .client
                .post(self.endpoint())
                .bearer_auth(&self.api_key)
                .json(&request)
                .send()
                .context("failed to call DeepSeek chat completions")?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().unwrap_or_default();
                return Err(anyhow!("DeepSeek request failed: {} {}", status, body));
            }

            let completion: ChatCompletionResponse = response.json()?;
            let content = completion
                .choices
                .first()
                .map(|choice| choice.message.content.trim())
                .ok_or_else(|| anyhow!("DeepSeek returned no choices"))?;
            if content.is_empty() {
                last_empty = true;
                continue;
            }
            let parsed: AiPitResponse =
                serde_json::from_str(content).context("failed to parse AI JSON output")?;
            validate_ai_pit_response(&parsed)?;
            return Ok(parsed);
        }

        if last_empty {
            Err(anyhow!("DeepSeek returned empty content twice"))
        } else {
            Err(anyhow!("DeepSeek returned no usable content"))
        }
    }
}

pub struct HeuristicProvider;

impl AiProvider for HeuristicProvider {
    fn extract_pit(&self, raw_text: &str, existing_sops: &[SopSummary]) -> Result<AiPitResponse> {
        if raw_text.contains("无效 AI 响应") {
            return Ok(AiPitResponse {
                classification: "pit".to_string(),
                confidence: 2.0,
                pit: None,
                sop_action: AiSopAction {
                    action_type: "invalid".to_string(),
                    sop_title: String::new(),
                    checklist_items: Vec::new(),
                    reason: "heuristic invalid response fixture".to_string(),
                },
            });
        }

        if raw_text.contains("普通笔记") {
            return Ok(AiPitResponse {
                classification: "note".to_string(),
                confidence: 0.9,
                pit: None,
                sop_action: AiSopAction {
                    action_type: "none".to_string(),
                    sop_title: String::new(),
                    checklist_items: Vec::new(),
                    reason: "heuristic note classification".to_string(),
                },
            });
        }

        let scenario = if raw_text.contains("客户") || raw_text.contains("交付") {
            "客户交付"
        } else if raw_text.contains("迁移")
            || raw_text.contains("migration")
            || raw_text.contains("数据库")
        {
            "数据库迁移"
        } else if raw_text.contains("发布")
            || raw_text.contains("上线")
            || raw_text.to_ascii_lowercase().contains("ci")
            || raw_text.to_ascii_lowercase().contains("release")
        {
            "发布流程"
        } else {
            "未分类"
        };
        let title = raw_text
            .chars()
            .take(28)
            .collect::<String>()
            .trim()
            .to_string();
        let sop_title = existing_sops
            .iter()
            .find(|sop| sop.scenarios.iter().any(|item| item.contains(scenario)))
            .map(|sop| sop.title.clone())
            .unwrap_or_else(|| format!("SOP - {}检查", scenario));
        let checklist = if raw_text.to_ascii_lowercase().contains("secret") {
            vec!["检查 CI secret 是否为最新值".to_string()]
        } else if raw_text.contains("客户") {
            vec!["检查测试账号、权限、测试数据和验收说明是否齐全".to_string()]
        } else if raw_text.contains("迁移") || raw_text.contains("数据库") {
            vec!["检查数据库迁移脚本的默认值、回滚方案和线上兼容性".to_string()]
        } else {
            vec!["把本次问题转成执行前检查项".to_string()]
        };

        let trigger_keywords = if scenario == "发布流程" {
            vec![
                "上线".to_string(),
                "发布".to_string(),
                "release".to_string(),
                "production".to_string(),
                "CI".to_string(),
            ]
        } else if scenario == "客户交付" {
            vec!["客户".to_string(), "交付".to_string(), "验收".to_string()]
        } else if scenario == "数据库迁移" {
            vec![
                "数据库".to_string(),
                "迁移".to_string(),
                "migration".to_string(),
            ]
        } else {
            vec![scenario.to_string()]
        };

        let no_sop_action = raw_text.contains("不需要 SOP");
        let needs_review_action = raw_text.contains("需要人工确认 SOP");
        Ok(AiPitResponse {
            classification: "pit".to_string(),
            confidence: 0.86,
            pit: Some(AiPitFields {
                title,
                scenario: scenario.to_string(),
                symptom: raw_text.to_string(),
                root_cause: "根据原始记录推断，需要人工复核根因".to_string(),
                fix: "按原始记录中的修复方式处理，并补充验证步骤".to_string(),
                prevention_rule: checklist[0].clone(),
                risk_level: "high".to_string(),
                recurrence_probability: "medium".to_string(),
                tags: vec!["pit".to_string()],
                trigger_keywords,
            }),
            sop_action: AiSopAction {
                action_type: if no_sop_action {
                    "none".to_string()
                } else if needs_review_action {
                    "needs_review".to_string()
                } else if existing_sops.iter().any(|sop| sop.title == sop_title) {
                    "update_existing".to_string()
                } else {
                    "create_new".to_string()
                },
                sop_title,
                checklist_items: checklist,
                reason: "heuristic provider fallback".to_string(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AiConfig;

    #[test]
    #[ignore]
    fn deepseek_smoke_extracts_structured_pit() {
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .expect("set DEEPSEEK_API_KEY before running this ignored smoke test");
        let config = AppConfig {
            vault_path: std::path::PathBuf::from("unused"),
            language: "zh-CN".to_string(),
            db_path_override: None,
            ai: AiConfig {
                provider: "deepseek".to_string(),
                model: "deepseek-v4-pro".to_string(),
                base_url: "https://api.deepseek.com".to_string(),
            },
        };
        let secrets = Secrets {
            deepseek_api_key: Some(api_key),
        };
        let provider = build_ai_provider(&config, &secrets).unwrap();
        let response = provider
            .extract_pit(
                "今天上线漏了 CI secret，导致 production 请求失败，后来更新 secret 修复。",
                &[],
            )
            .unwrap();

        validate_ai_pit_response(&response).unwrap();
        assert_eq!(response.classification, "pit");
        let pit = response.pit.unwrap();
        assert!(!pit.title.trim().is_empty());
        assert!(
            !response.sop_action.checklist_items.is_empty()
                || !pit.prevention_rule.trim().is_empty()
        );
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    response_format: serde_json::Value,
    thinking: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: String,
}
