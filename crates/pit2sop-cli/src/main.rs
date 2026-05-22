use anyhow::Result;
use clap::{Parser, Subcommand};
use pit2sop_core::{AppConfig, Pit2Sop};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Parser)]
#[command(name = "sop")]
#[command(about = "Pit2SOP local CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init {
        vault_path: Option<PathBuf>,
    },
    Pit {
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
    },
    #[command(alias = "check")]
    Doing {
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
    },
    Search {
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,
    },
    Open,
    Reindex,
    Config,
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => run_interactive()?,
        Some(Commands::Init { vault_path }) => {
            let config = Pit2Sop::init(vault_path)?;
            let out = Output::new(&config);
            out.init_done(&config);
        }
        Some(Commands::Pit { text }) => {
            let app = Pit2Sop::load()?;
            let out = Output::new(&app.config);
            let text = join_args(text);
            match app.process_pit(&text) {
                Ok(summary) => out.pit_done(&summary),
                Err(error) => {
                    out.error(&error.to_string());
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Doing { text }) => {
            let app = Pit2Sop::load()?;
            let out = Output::new(&app.config);
            let text = join_args(text);
            let matches = app.doing(&text)?;
            out.doing(&matches);
        }
        Some(Commands::Search { query }) => {
            let app = Pit2Sop::load()?;
            let out = Output::new(&app.config);
            let query = join_args(query);
            let results = app.search(&query)?;
            out.search(&results);
        }
        Some(Commands::Open) => {
            let app = Pit2Sop::load()?;
            open_vault(&app.config)?;
        }
        Some(Commands::Reindex) => {
            let app = Pit2Sop::load()?;
            let out = Output::new(&app.config);
            let count = app.reindex()?;
            out.reindex(count);
        }
        Some(Commands::Config) => {
            let config = AppConfig::load_or_default()?;
            println!("{}", toml_like_config(&config));
        }
        Some(Commands::Status) => {
            let app = Pit2Sop::load()?;
            let out = Output::new(&app.config);
            out.status(&app.status()?);
        }
    }

    Ok(())
}

fn join_args(args: Vec<String>) -> String {
    args.join(" ")
}

fn run_interactive() -> Result<()> {
    loop {
        let app = Pit2Sop::load()?;
        let out = Output::new(&app.config);
        println!();
        if out.english {
            println!("Pit2SOP");
            println!("1. Record a pit");
            println!("2. Check what I am about to do");
            println!("3. Search SOP/Pit");
            println!("4. Open Vault");
            println!("5. Reindex Vault");
            println!("q. Quit");
        } else {
            println!("Pit2SOP");
            println!("1. 记录一个坑");
            println!("2. 我要做一件事");
            println!("3. 搜索 SOP / Pit");
            println!("4. 打开 Vault");
            println!("5. 重建索引");
            println!("q. 退出");
        }

        let choice = prompt(if out.english { "Choice" } else { "选择" })?;
        match choice.trim() {
            "1" => {
                let text = prompt(if out.english {
                    "Pit text"
                } else {
                    "把坑写下来"
                })?;
                if text.trim().is_empty() {
                    continue;
                }
                match app.process_pit(&text) {
                    Ok(summary) => out.pit_done(&summary),
                    Err(error) => out.error(&error.to_string()),
                }
            }
            "2" => {
                let text = prompt(if out.english {
                    "What are you doing"
                } else {
                    "你要做什么"
                })?;
                if text.trim().is_empty() {
                    continue;
                }
                let matches = app.doing(&text)?;
                out.doing(&matches);
            }
            "3" => {
                let query = prompt(if out.english { "Search" } else { "搜索" })?;
                if query.trim().is_empty() {
                    continue;
                }
                let results = app.search(&query)?;
                out.search(&results);
            }
            "4" => open_vault(&app.config)?,
            "5" => {
                let count = app.reindex()?;
                out.reindex(count);
            }
            "q" | "Q" => break,
            _ => {
                if out.english {
                    println!("Unknown choice.");
                } else {
                    println!("无效选择。");
                }
            }
        }
    }

    Ok(())
}

fn prompt(label: &str) -> Result<String> {
    print!("{}: ", label);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn open_vault(config: &AppConfig) -> Result<()> {
    let path = &config.vault_path;
    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()?
    } else if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path).status()?
    } else {
        Command::new("xdg-open").arg(path).status()?
    };

    if status.success() {
        println!("Vault: {}", path.display());
    } else {
        anyhow::bail!("failed to open vault: {}", path.display());
    }
    Ok(())
}

struct Output {
    english: bool,
}

impl Output {
    fn new(config: &AppConfig) -> Self {
        Self {
            english: config.language.to_ascii_lowercase().starts_with("en"),
        }
    }

    fn init_done(&self, config: &AppConfig) {
        if self.english {
            println!("Initialized Pit2SOP");
            println!("Vault: {}", config.vault_path.display());
            println!("Provider: {}", config.ai.provider);
            println!("Model: {}", config.ai.model);
        } else {
            println!("Pit2SOP 初始化完成");
            println!("Vault: {}", config.vault_path.display());
            println!("AI provider: {}", config.ai.provider);
            println!("Model: {}", config.ai.model);
        }
    }

    fn pit_done(&self, summary: &pit2sop_core::models::ProcessingSummary) {
        if self.english {
            println!("Status: {:?}", summary.status);
            println!("Capture: {}", summary.capture_id);
            if let Some(path) = &summary.pit_path {
                println!("Pit: {}", path.display());
            }
            if let Some(path) = &summary.sop_path {
                println!("SOP: {}", path.display());
            }
            if let Some(path) = &summary.pending_patch_path {
                println!("Pending patch: {}", path.display());
            }
            println!("{}", summary.message);
        } else {
            println!("状态：{:?}", summary.status);
            println!("Capture：{}", summary.capture_id);
            if let Some(path) = &summary.pit_path {
                println!("Pit：{}", path.display());
            }
            if let Some(path) = &summary.sop_path {
                println!("SOP：{}", path.display());
            }
            if let Some(path) = &summary.pending_patch_path {
                println!("待确认 patch：{}", path.display());
            }
            println!("{}", summary.message);
        }
    }

    fn doing(&self, matches: &[pit2sop_core::models::DoingMatch]) {
        if matches.is_empty() {
            if self.english {
                println!("No SOP matched above the reminder threshold.");
            } else {
                println!("没有命中提醒阈值以上的 SOP。");
            }
            return;
        }

        for item in matches {
            if self.english {
                println!("Recommended SOP: {}", item.title);
                println!("Score: {:.2}", item.score);
                println!("Risk: {:?}", item.risk);
                println!("Path: {}", item.path);
                println!("Reason: {}", item.reasons.join("；"));
                if !item.checklist_items.is_empty() {
                    println!("Do now:");
                    for checklist_item in &item.checklist_items {
                        println!("- [ ] {}", checklist_item);
                    }
                }
            } else {
                println!("推荐 SOP：{}", item.title);
                println!("分数：{:.2}", item.score);
                println!("风险：{:?}", item.risk);
                println!("路径：{}", item.path);
                println!("原因：{}", item.reasons.join("；"));
                if !item.checklist_items.is_empty() {
                    println!("现在检查：");
                    for checklist_item in &item.checklist_items {
                        println!("- [ ] {}", checklist_item);
                    }
                }
            }
            println!();
        }
    }

    fn search(&self, results: &[pit2sop_core::models::SearchResult]) {
        if results.is_empty() {
            if self.english {
                println!("No results.");
            } else {
                println!("没有搜索结果。");
            }
            return;
        }

        for result in results {
            println!("[{}] {}", result.doc_type, result.title);
            println!("{}", result.path);
            if !result.snippet.trim().is_empty() {
                println!("{}", result.snippet);
            }
            println!();
        }
    }

    fn reindex(&self, count: usize) {
        if self.english {
            println!("Indexed {} Markdown documents.", count);
        } else {
            println!("已索引 {} 个 Markdown 文档。", count);
        }
    }

    fn status(&self, status: &pit2sop_core::models::AppStatus) {
        if self.english {
            println!("Vault: {}", status.vault_path.display());
            println!("DB: {}", status.db_path.display());
            println!("AI provider: {}", status.ai_provider);
            println!("Model: {}", status.ai_model);
            println!(
                "Secrets: {}",
                if status.secrets_configured {
                    "configured"
                } else {
                    "missing"
                }
            );
            println!("Indexed docs: {}", status.indexed_docs);
            println!("Pits: {}", status.pit_files);
            println!("SOPs: {}", status.sop_files);
        } else {
            println!("Vault：{}", status.vault_path.display());
            println!("DB：{}", status.db_path.display());
            println!("AI provider：{}", status.ai_provider);
            println!("Model：{}", status.ai_model);
            println!(
                "Secrets：{}",
                if status.secrets_configured {
                    "已配置"
                } else {
                    "缺失"
                }
            );
            println!("已索引文档：{}", status.indexed_docs);
            println!("Pits：{}", status.pit_files);
            println!("SOPs：{}", status.sop_files);
        }
    }

    fn error(&self, message: &str) {
        if self.english {
            eprintln!("Error: {}", message);
        } else {
            eprintln!("错误：{}", message);
        }
    }
}

fn toml_like_config(config: &AppConfig) -> String {
    format!(
        "vault_path = \"{}\"\nlanguage = \"{}\"\n\n[ai]\nprovider = \"{}\"\nmodel = \"{}\"\nbase_url = \"{}\"",
        config.vault_path.display(),
        config.language,
        config.ai.provider,
        config.ai.model,
        config.ai.base_url
    )
}
