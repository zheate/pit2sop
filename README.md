# Pit2SOP

Pit2SOP turns repeated mistakes into pre-action checklists.

中文定义：Pit2SOP 把踩过的坑变成下次行动前的检查清单。

Core loop:

```text
Capture a pit
→ extract the prevention rule
→ update an SOP
→ before similar work, surface the checklist
→ reduce repeated mistakes
```

Pit2SOP is not a general note app, task manager, knowledge base, or chat assistant. Its value is whether it reminds you before repeating a real mistake.

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

## Desktop V0.2 Alpha

The desktop app wraps `pit2sop-core` directly through Tauri commands. It does not run the CLI as a sidecar.

Run the local shell:

```bash
cd apps/desktop
npm install
npm run tauri dev
```

The current desktop settings loop supports:

- Choose and save the Vault path.
- Initialize the Vault directory structure while saving settings.
- Save or clear the DeepSeek API key through Rust only.
- Test the active AI provider.
- Open the configured Vault.

API keys are stored in `~/.pit2sop/secrets.toml`. The frontend only receives `configured` / `missing` state.

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

V0.2 desktop smoke path:

```text
Settings -> choose/save Vault -> save/test AI key -> record Pit -> apply/reject Pending -> doing -> search -> restart app
```

Use `pit2sop_docs/desktop-smoke-checklist.md` before tagging a desktop alpha or beta.
Use `pit2sop_docs/release-artifacts.md` for the current macOS artifact commands.
Use `pit2sop_docs/macos-signing-notarization.md` before attempting a signed macOS release.

## Status

V0.1 is the local CLI/core review loop. V0.2 alpha is the Tauri desktop shell and settings loop. It has no phone app, background agent, or system notification yet.
