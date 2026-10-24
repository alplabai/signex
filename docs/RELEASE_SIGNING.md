# Release signing

`.github/workflows/release.yml` signs the Windows and macOS artifacts when the
repository secrets below exist. With no secrets the build is unchanged:
unsigned installers on Windows and an ad-hoc signature on macOS. Linux is not
signed. Release mechanics (tagging, CHANGELOG) are in the
`cutting-a-signex-release` procedure and `CONTRIBUTING.md`.

Secrets are never echoed: they are passed through step `env:` only, the
decoded `.pfx` / `.p12` live under `$RUNNER_TEMP`, and `always()` steps delete
the `.pfx` and the temporary keychain even when a step fails.

## Secrets

Add under Settings > Secrets and variables > Actions.

| Secret | Gate | Value |
|---|---|---|
| `WINDOWS_CERT_PFX` | Windows signing | base64 of the code-signing `.pfx` |
| `WINDOWS_CERT_PASSWORD` | Windows signing | the `.pfx` export password |
| `MACOS_CERT_P12` | macOS signing | base64 of the "Developer ID Application" `.p12` |
| `MACOS_CERT_PASSWORD` | macOS signing | the `.p12` export password |
| `MACOS_SIGN_IDENTITY` | optional | identity name or SHA-1; default: first "Developer ID Application" identity in the `.p12` |
| `APPLE_ID` | notarization | Apple ID email of the developer account |
| `APPLE_TEAM_ID` | notarization | 10-character Team ID |
| `APPLE_APP_PASSWORD` | notarization | app-specific password for that Apple ID |

Windows signing needs both Windows secrets. macOS signing needs both macOS
cert secrets; notarization additionally needs all three `APPLE_*` secrets. A
signed-but-not-notarized macOS build emits a workflow warning and still
publishes (Gatekeeper will warn).

## Creating them

Windows: obtain an Authenticode code-signing certificate (OV or EV from a CA).
Export it with its private key as `.pfx`, then
`base64 -w0 cert.pfx` (Linux) or
`[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))` (PowerShell)
and store the output. EV certificates held on a hardware token cannot be
exported and do not work with this file-based flow; use an OV certificate or a
cloud-signing service (would need a workflow change).

macOS: in the Apple Developer account create a "Developer ID Application"
certificate. In Keychain Access export the certificate with its private key as
`.p12` (set a password), then `base64 -i cert.p12 | pbcopy`. Create the
app-specific password at appleid.apple.com > Sign-In and Security.

## What the workflow does

- Windows (`installer/windows/sign.ps1`): `signtool sign /fd SHA256 /tr
  http://timestamp.digicert.com /td SHA256` on `signex.exe` (before the
  installer and the portable ZIP are built, so both carry the signed exe), then
  on the InnoSetup installer. Each file is checked with `signtool verify /pa`.
  signtool comes from the runner's Windows SDK.
- macOS (`installer/macos/build-dmg.sh` + workflow): the `.p12` is imported into
  a temporary keychain; the executable and then the `.app` are signed with
  `--options runtime --timestamp` (hardened runtime; inside-out, no `--deep`);
  the DMG is built and signed; `xcrun notarytool submit --wait` notarizes the
  DMG; `xcrun stapler staple` attaches the ticket. No entitlements are used
  (no JIT or unsigned plugins; wgpu/Metal needs no exception). If a future
  feature needs one, set `MACOS_ENTITLEMENTS` for `build-dmg.sh`.

## Verifying a release artifact

```
signtool verify /pa /v signex-setup-x86_64-<v>.exe     # Windows (also signex.exe in the ZIP)
codesign --verify --deep --strict --verbose=2 /Applications/Signex.app
spctl -a -vv /Applications/Signex.app                  # expect: accepted, source=Notarized Developer ID
spctl -a -vv -t open --context context:primary-signature signex-macos-aarch64-<v>.dmg
xcrun stapler validate signex-macos-aarch64-<v>.dmg
```

## Not verified in CI authoring

The signing path has not run against real certificates; the first tagged
release after the secrets are added is its first real test (a failed leg means
no GitHub Release for that tag, and tags cannot be re-used). Consider a
pre-release tag such as `vX.Y.Z-rc.1` first.
