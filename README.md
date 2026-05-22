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
6. 待确认 Patch
```

## CLI Commands

Record a pit:

```bash
sop pit 今天上线漏了 CI secret，导致 production 请求失败。
```

Check what you are about to do:

```bash
sop check 我要上线 2.5.0
```

Search generated knowledge:

```bash
sop search secret
```

Review pending SOP patches:

```bash
sop pending
sop apply-patch "99_System/Pending Patches/2026-05-22-134500 SOP - 发布流程检查 abc12345.md"
sop reject-patch "99_System/Pending Patches/2026-05-22-134500 SOP - 发布流程检查 abc12345.md"
```

Open the configured vault:

```bash
sop open
```

Rebuild SQLite search cache from Markdown:

```bash
sop reindex
```

Inspect local configuration and cache status:

```bash
sop status
```

Use an environment variable for one-off DeepSeek calls:

```bash
DEEPSEEK_API_KEY=your-key-here sop pit 今天上线漏了 CI secret
```

## Architecture

```text
crates/pit2sop-cli   -> terminal UI and commands
crates/pit2sop-core  -> AI, matching, Markdown, SQLite, config
apps/desktop         -> Tauri 2 desktop shell
pit2sop_docs         -> product, architecture, and engineering specs
```

Core rules:

- Obsidian Markdown is the source of truth.
- SQLite stores only status and search cache.
- AI returns structured JSON; Rust code decides what gets written.
- `sop_action.type = none` writes the Pit as `processed`; explicit pending patches still require apply/reject to close review.
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
cargo fmt --check
cargo test
cargo clippy --workspace --all-targets -- -D warnings
```

Optional DeepSeek smoke test:

```bash
DEEPSEEK_API_KEY=your-key-here cargo test deepseek_smoke -- --ignored
```

## Status

V0.1 RC is a local CLI-first build. It has no desktop tray, phone app, background agent, or system notification yet.
