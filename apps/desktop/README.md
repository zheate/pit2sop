# Pit2SOP Desktop

Tauri 2 shell for the Pit2SOP Rust core.

Current V0.2 scope:

- record a Pit
- check a doing prompt
- search Pit / SOP cache
- list, apply, and reject pending patches
- inspect local status

Run from this directory:

```bash
npm install
npm run tauri dev
```

Build the macOS app:

```bash
npm run tauri build -- --bundles app
```
