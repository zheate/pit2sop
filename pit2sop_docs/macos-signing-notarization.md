# macOS Signing And Notarization Plan

Status: beta.2 spike plan. `v0.2.0-beta.1` is intentionally unsigned and not notarized.

## Goal

Ship a macOS DMG that Gatekeeper can identify as Developer ID signed and notarized.

Last released beta artifact:

```text
target/release/bundle/dmg/Pit2SOP_0.2.0-beta.1_aarch64.dmg
```

Expected beta.2 DMG path:

```text
target/release/bundle/dmg/Pit2SOP_0.2.0-beta.2_aarch64.dmg
```

Current project state:

```text
productName: Pit2SOP
bundle identifier: com.pit2sop.desktop
bundle target: dmg
signing identity: not configured
notarization credentials: not configured
```

## External Requirements

- Apple Developer Program membership.
- Developer ID Application certificate for distributing outside the Mac App Store.
- Apple notarization credentials.
- Xcode command line tools with `notarytool` and `stapler`.

Tauri's macOS signing guide states that signing is needed to avoid browser-downloaded apps being reported as broken or unable to start. It also documents `Developer ID Application` as the certificate type for distribution outside the Mac App Store and supports `APPLE_SIGNING_IDENTITY` as an environment variable.

Apple's notarization docs describe notarization as an automated Apple scan for malicious content and code-signing issues. Apple also documents that Developer ID-distributed software should use `notarytool` for custom notarization workflows, and that notarized software can receive a ticket that is stapled to the app or package.

References:

- https://v2.tauri.app/distribute/sign/macos/
- https://developer.apple.com/developer-id/
- https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution
- https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution/customizing_the_notarization_workflow

## Local Spike

Find available signing identities:

```bash
security find-identity -v -p codesigning
```

Build a DMG with a local Developer ID identity:

```bash
cd apps/desktop
APPLE_SIGNING_IDENTITY="Developer ID Application: <Name> (<TEAM_ID>)" \
  npm run tauri build -- --bundles dmg
```

Verify the app signature:

```bash
codesign --verify --deep --strict --verbose=2 \
  ../../target/release/bundle/macos/Pit2SOP.app

codesign --display --verbose=4 \
  ../../target/release/bundle/macos/Pit2SOP.app

spctl --assess --type execute --verbose \
  ../../target/release/bundle/macos/Pit2SOP.app
```

Notarize the DMG with an Apple ID profile:

```bash
xcrun notarytool store-credentials pit2sop-notary \
  --apple-id "<APPLE_ID>" \
  --team-id "<APPLE_TEAM_ID>" \
  --password "<APP_SPECIFIC_PASSWORD>"

xcrun notarytool submit \
  ../../target/release/bundle/dmg/Pit2SOP_0.2.0-beta.2_aarch64.dmg \
  --keychain-profile pit2sop-notary \
  --wait
```

Staple and validate:

```bash
xcrun stapler staple \
  ../../target/release/bundle/dmg/Pit2SOP_0.2.0-beta.2_aarch64.dmg

xcrun stapler validate \
  ../../target/release/bundle/dmg/Pit2SOP_0.2.0-beta.2_aarch64.dmg

spctl --assess --type open --verbose \
  ../../target/release/bundle/dmg/Pit2SOP_0.2.0-beta.2_aarch64.dmg
```

## GitHub Actions Secrets

For a future signed workflow, use secrets only. Do not commit certificates or app-specific passwords.

Required secrets:

```text
APPLE_ID
APPLE_TEAM_ID
APPLE_PASSWORD
APPLE_CERTIFICATE
APPLE_CERTIFICATE_PASSWORD
KEYCHAIN_PASSWORD
```

`APPLE_CERTIFICATE` should be a base64 encoded `.p12` export of the Developer ID Application certificate.

## CI Shape

Keep the current Linux CI as the fast correctness gate. Add a separate manual macOS signing workflow later:

```text
workflow_dispatch
  -> runs-on: macos-latest
  -> import Developer ID cert into temporary keychain
  -> npm ci
  -> npm run tauri build -- --bundles dmg
  -> notarize
  -> staple
  -> upload artifact
```

Do not run this on every push until secrets, cost, and failure modes are understood.

## Beta.2 Acceptance

- Local signing command documented.
- At least one signed local `.app` or `.dmg` inspected with `codesign`.
- Notarization credentials path documented.
- If notarization is not completed in beta.2, release notes must continue to say the DMG is not notarized.
