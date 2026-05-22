# Pit2SOP

Pit2SOP turns daily mistakes into executable personal SOP checklists, then reminds you before similar work.

Current V0.1 is intentionally small:

- CLI-only
- Obsidian Markdown as the source of truth
- SQLite as rebuildable cache/index
- DeepSeek JSON extraction
- Local SOP matching for `doing` prompts

## Quick Start

Install the CLI from this repo:

```bash
cargo install --path crates/pit2sop-cli --force
```

Initialize a local Obsidian vault:

```bash
sop init /Users/zh/Documents/test/pit2sop/test-vault
```

Configure the DeepSeek API key locally:

```toml
# ~/.pit2sop/secrets.toml
deepseek_api_key = "your-key-here"
```

Then run:

```bash
sop
```

The interactive menu supports:

```text
1. 记录一个坑
2. 我要做一件事
3. 搜索 SOP / Pit
4. 打开 Vault
5. 重建索引
```

## CLI Commands

Record a pit:

```bash
sop pit "今天上线漏了 CI secret，导致 production 请求失败。"
```

Check what you are about to do:

```bash
sop doing "我要上线 2.5.0"
```

Search generated knowledge:

```bash
sop search secret
```

Open the configured vault:

```bash
sop open
```

Rebuild SQLite search cache from Markdown:

```bash
sop reindex
```

## Architecture

```text
crates/pit2sop-cli   -> terminal UI and commands
crates/pit2sop-core  -> AI, matching, Markdown, SQLite, config
pit2sop_docs         -> product, architecture, and engineering specs
```

Core rules:

- Obsidian Markdown is the source of truth.
- SQLite stores only status and search cache.
- AI returns structured JSON; Rust code decides what gets written.
- Existing SOPs are only auto-updated inside the marker block:

```markdown
<!-- pit2sop:start:auto-items -->
<!-- pit2sop:end:auto-items -->
```

## Local Files Not Committed

Do not commit:

- `test-vault/`
- `target/`
- `.DS_Store`
- `~/.pit2sop/secrets.toml`
- `~/.pit2sop/cache/**`

The repository includes examples only. Real keys and personal SOP data stay local.

## Development

Run tests:

```bash
cargo test
```

Format:

```bash
cargo fmt
```

## Status

V0.1 is a local CLI prototype. It has no desktop tray, phone app, background agent, or system notification yet.
