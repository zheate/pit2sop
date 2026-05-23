# Release Artifacts

Current beta scope is macOS only.

## macOS

Build a local app bundle for direct launch:

```bash
cd apps/desktop
npm run tauri build -- --bundles app
```

Build a user-installable DMG:

```bash
cd apps/desktop
npm run tauri build -- --bundles dmg
```

Expected outputs:

```text
target/release/bundle/macos/Pit2SOP.app
target/release/bundle/dmg/Pit2SOP_0.2.0-beta.1_*.dmg
```

## Not In Beta Scope

Windows and Linux installers are intentionally deferred until the desktop macOS flow is stable.

