# Desktop Smoke Checklist

Use this checklist before tagging a desktop alpha or beta release.

For a non-destructive smoke run, prefer an isolated home instead of moving the real config:

```bash
SMOKE_ROOT="$(mktemp -d /tmp/pit2sop-smoke.XXXXXX)"
mkdir -p "$SMOKE_ROOT/vault"
PIT2SOP_HOME="$SMOKE_ROOT/home" target/release/bundle/macos/Pit2SOP.app/Contents/MacOS/pit2sop-desktop
```

## Fresh User Setup

1. Quit Pit2SOP.
2. Move the current local config out of the way:

```bash
mv ~/.pit2sop ~/.pit2sop.backup
```

3. Open `Pit2SOP.app`.
4. Confirm Settings shows missing setup state for saved config, Vault, and DeepSeek key.
5. Choose an existing empty directory as the Vault path.
6. Save Settings.
7. Confirm the Vault directory now contains:

```text
00_Inbox/
01_Pits/
02_SOPs/
99_System/
```

8. Save a DeepSeek API key.
9. Confirm the UI shows `configured` and never echoes the key.
10. Run AI test and confirm it returns available.

## Core Desktop Loop

1. Record a pit from the desktop UI.
2. Confirm a Pit markdown file is created.
3. If a pending patch is generated, apply it.
4. Confirm Pending refreshes and the patch disappears from the pending list.
5. Run Doing with a related task.
6. Confirm Doing returns the generated or updated SOP checklist.
7. Run Search with a keyword from the Pit.
8. Confirm Search returns the Pit or SOP.

## Secret And Restart Behavior

1. Clear the API key.
2. Run AI test and confirm the error is explicit.
3. Quit and reopen `Pit2SOP.app`.
4. Confirm saved Settings persist.
5. Restore the original config if needed:

```bash
rm -rf ~/.pit2sop
mv ~/.pit2sop.backup ~/.pit2sop
```

## Smoke Results

### 2026-05-23 beta.1 prep local smoke

Environment:

- macOS app bundle: `target/release/bundle/macos/Pit2SOP.app`
- DMG artifact: `target/release/bundle/dmg/Pit2SOP_0.2.0-beta.1_aarch64.dmg`
- Isolated home: `/tmp/pit2sop-beta-smoke.5b1qdk/home`
- Vault: `/tmp/pit2sop-beta-smoke.5b1qdk/vault`
- Provider: DeepSeek `deepseek-v4-pro`

Result:

- Fresh onboarding: pass. Missing config and missing DeepSeek key were shown before setup.
- Settings save: pass. Vault path persisted under the isolated `PIT2SOP_HOME`.
- Vault init: pass. Required Pit2SOP directories were created.
- AI health: pass. DeepSeek provider returned available; key was shown only as configured.
- Pit capture: pass. Desktop UI created a Pit and `SOP - PBS安装方向检查.md`.
- Doing: pass. `我要装 PBS` matched the generated PBS SOP checklist.
- Search: pass. `PBS` returned the generated Pit and SOP from the SQLite cache.
- Pending: pass for empty state. No pending patch was generated in this DeepSeek run.
- Restart persistence: pass. Relaunching with the same isolated home preserved settings and secret status.

Notes:

- The API key was not typed into the UI during this run; an existing local `secrets.toml` was copied into the isolated smoke home to avoid exposing the secret in automation logs.
- The local heuristic provider was also checked for mechanical write flow. It created a generic `SOP - 未分类检查.md`; semantic `PBS` matching should be judged with DeepSeek, not heuristic.
