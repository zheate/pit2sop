use crate::models::{DoingMatch, RiskLevel, SopSummary};
use std::collections::HashSet;

pub fn match_doing(text: &str, sops: &[SopSummary]) -> Vec<DoingMatch> {
    let lowered = text.to_ascii_lowercase();
    let mut matches = sops
        .iter()
        .filter_map(|sop| {
            let mut score = 0.0_f32;
            let mut reasons = Vec::new();

            for scenario in &sop.scenarios {
                if contains_case_aware(text, &lowered, scenario) {
                    score += 0.30;
                    reasons.push(format!("命中场景：{}", scenario));
                }
            }

            for trigger in &sop.triggers {
                if contains_case_aware(text, &lowered, trigger) {
                    score += 0.65;
                    reasons.push(format!("命中关键词：{}", trigger));
                } else if let Some(token) = fuzzy_acronym_match(text, trigger) {
                    score += 0.65;
                    reasons.push(format!("疑似关键词：{}≈{}", token, trigger));
                }
            }

            let title_signal = sop
                .title
                .replace("SOP", "")
                .replace('-', "")
                .replace("检查", "");
            if !title_signal.trim().is_empty() && text.contains(title_signal.trim()) {
                score += 0.20;
                reasons.push(format!("命中 SOP 标题：{}", sop.title));
            }

            let sop_blob = format!(
                "{} {} {}",
                sop.title,
                sop.scenarios.join(" "),
                sop.triggers.join(" ")
            );
            let shared_signals = shared_domain_signals(text, &sop_blob);
            if !shared_signals.is_empty() {
                score += 0.55 + ((shared_signals.len() - 1) as f32 * 0.10);
                reasons.push(format!("命中领域信号：{}", shared_signals.join("、")));
            }

            if sop.risk_level == RiskLevel::High && score > 0.0 {
                score += 0.10;
                reasons.push("高风险 SOP 加权".to_string());
            }

            let capped = score.min(1.0);
            if capped < 0.65 {
                return None;
            }

            Some(DoingMatch {
                score: capped,
                title: sop.title.clone(),
                path: sop.obsidian_path.to_string_lossy().to_string(),
                risk: sop.risk_level.clone(),
                scenarios: sop.scenarios.clone(),
                checklist_items: sop.checklist_items.clone(),
                reasons,
            })
        })
        .collect::<Vec<_>>();

    matches.sort_by(|a, b| b.score.total_cmp(&a.score));
    matches
}

fn contains_case_aware(original: &str, lowered: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    if needle.is_ascii() {
        lowered.contains(&needle.to_ascii_lowercase())
    } else {
        original.contains(needle)
    }
}

fn fuzzy_acronym_match(text: &str, trigger: &str) -> Option<String> {
    let trigger = trigger.trim();
    if !is_short_acronym(trigger) {
        return None;
    }

    extract_ascii_tokens(text)
        .into_iter()
        .find(|token| token.len() == trigger.len() && hamming_distance(token, trigger) == 1)
}

fn is_short_acronym(value: &str) -> bool {
    let len = value.chars().count();
    (3..=6).contains(&len) && value.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn extract_ascii_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_uppercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn hamming_distance(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .filter(|(left, right)| !left.eq_ignore_ascii_case(right))
        .count()
}

fn shared_domain_signals(left: &str, right: &str) -> Vec<String> {
    let left_signals = domain_signals(left);
    let right_signals = domain_signals(right);
    let mut shared = left_signals
        .intersection(&right_signals)
        .cloned()
        .collect::<Vec<_>>();
    shared.sort();
    shared
}

fn domain_signals(value: &str) -> HashSet<String> {
    let lowered = value.to_ascii_lowercase();
    [
        ("上线", &["上线"][..]),
        ("发布", &["发布", "release"]),
        ("部署", &["部署", "deploy"]),
        ("生产", &["生产", "production"]),
        ("CI", &["ci"]),
        ("secret", &["secret"]),
        ("数据库", &["数据库", "database", "db"]),
        ("迁移", &["迁移", "migration"]),
        ("默认值", &["默认值", "default"]),
        ("客户", &["客户"]),
        ("交付", &["交付", "delivery"]),
        ("验收", &["验收"]),
        ("测试账号", &["测试账号", "test account"]),
        ("PBS", &["pbs"]),
        ("装反", &["装反"]),
        ("组装", &["组装", "装配"]),
        ("蓝光", &["蓝光"]),
        ("拨片", &["拨片"]),
        ("大反", &["大反"]),
        ("SACK", &["sack"]),
    ]
    .into_iter()
    .filter_map(|(signal, aliases)| {
        let matched = aliases.iter().any(|alias| {
            if alias.is_ascii() {
                lowered.contains(alias)
            } else {
                value.contains(alias)
            }
        });
        matched.then(|| signal.to_string())
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn high_risk_trigger_crosses_cli_threshold() {
        let sops = vec![SopSummary {
            id: "sop_release".into(),
            title: "SOP - 发布流程检查".into(),
            status: "draft".into(),
            risk_level: RiskLevel::High,
            scenarios: vec!["发布流程".into()],
            triggers: vec!["上线".into(), "release".into()],
            related_pits: vec![],
            checklist_items: vec!["检查发布配置".into()],
            obsidian_path: PathBuf::from("02_SOPs/Release/SOP - 发布流程检查.md"),
        }];

        let matches = match_doing("我要上线 2.5.0", &sops);
        assert_eq!(matches.len(), 1);
        assert!(matches[0].score >= 0.65);
    }

    #[test]
    fn domain_signals_match_short_doing_text() {
        let sops = vec![
            SopSummary {
                id: "sop_release".into(),
                title: "SOP - 上线前CI/CD配置检查清单".into(),
                status: "draft".into(),
                risk_level: RiskLevel::High,
                scenarios: vec!["部署到生产环境时遗漏CI secret配置".into()],
                triggers: vec!["secret".into(), "CI".into(), "生产线".into()],
                related_pits: vec![],
                checklist_items: vec!["检查 CI secret".into()],
                obsidian_path: PathBuf::from("02_SOPs/Release/SOP - 上线前CI CD配置检查清单.md"),
            },
            SopSummary {
                id: "sop_db".into(),
                title: "SOP - 数据库迁移前检查默认值与兼容性".into(),
                status: "draft".into(),
                risk_level: RiskLevel::High,
                scenarios: vec!["数据库迁移时未设置默认值".into()],
                triggers: vec!["数据库迁移".into(), "默认值".into()],
                related_pits: vec![],
                checklist_items: vec!["检查默认值".into()],
                obsidian_path: PathBuf::from(
                    "02_SOPs/Development/SOP - 数据库迁移前检查默认值与兼容性.md",
                ),
            },
        ];

        assert_eq!(
            match_doing("我要上线 2.5.0", &sops)[0].title,
            "SOP - 上线前CI/CD配置检查清单"
        );
        assert_eq!(
            match_doing("我要改数据库 migration", &sops)[0].title,
            "SOP - 数据库迁移前检查默认值与兼容性"
        );
    }

    #[test]
    fn exact_pbs_trigger_is_enough_to_remind() {
        let sops = vec![SopSummary {
            id: "sop_pbs".into(),
            title: "SOP - 蓝光PBS组装方向检查".into(),
            status: "draft".into(),
            risk_level: RiskLevel::Medium,
            scenarios: vec!["操作员将PBS装反".into()],
            triggers: vec!["PBS".into(), "装反".into(), "拨片".into(), "大反".into()],
            related_pits: vec![],
            checklist_items: vec!["组装PBS前确认拨片朝向大反方向".into()],
            obsidian_path: PathBuf::from("02_SOPs/Personal Workflow/SOP - 蓝光PBS组装方向检查.md"),
        }];

        let matches = match_doing("我要装 PBS", &sops);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].title, "SOP - 蓝光PBS组装方向检查");
    }

    #[test]
    fn fuzzy_pbs_typo_still_reminds() {
        let sops = vec![SopSummary {
            id: "sop_pbs".into(),
            title: "SOP - 蓝光PBS组装方向检查".into(),
            status: "draft".into(),
            risk_level: RiskLevel::Medium,
            scenarios: vec!["操作员将PBS装反".into()],
            triggers: vec!["PBS".into(), "装反".into(), "拨片".into(), "大反".into()],
            related_pits: vec![],
            checklist_items: vec!["组装PBS前确认拨片朝向大反方向".into()],
            obsidian_path: PathBuf::from("02_SOPs/Personal Workflow/SOP - 蓝光PBS组装方向检查.md"),
        }];

        let matches = match_doing("我要装 PPS。", &sops);
        assert_eq!(matches.len(), 1);
        assert!(
            matches[0]
                .reasons
                .iter()
                .any(|reason| reason.contains("PPS≈PBS"))
        );
    }
}
