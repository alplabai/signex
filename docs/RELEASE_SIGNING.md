# Release signing

`.github/workflows/release.yml` signs the Windows and macOS artifacts when the
repository secrets below exist. With no secrets the build is unchanged:
unsigned installers on Windows and an ad-hoc signature on macOS. Linux is not
signed. Release mechanics (tagging, CHANGELOG) are in `CONTRIBUTING.md`.

## Job graph

```
version -> verify-version (tags only) -> test (tags only) -> build (matrix, cargo, NO signing material)
build -> package-windows (x64, arm64)  -> release (tags only)
build -> package-macos                 -> release (tags only)
build -> release (tags only)           # Linux legs: .deb, AppImage, tar.gz, built and uploaded in `build`
version -> sbom (read-only token)      -> release (tags only)
```

The Linux legs are x86_64 (`ubuntu-22.04`) and aarch64 (`ubuntu-22.04-arm`,
built natively, not cross-compiled). They stay on 22.04 so the binary needs
glibc 2.35 at most; `scripts/check_glibc_ceiling.sh` fails the leg if a newer
glibc symbol slips in (#1688). They sign nothing; `SHA256SUMS.txt` is the
only integrity artifact.

`build` compiles (every dependency `build.rs` runs there) and only hands the
unsigned `signex.exe` / `signex` to the `package-*` jobs as `unsigned-*`
artifacts. The `package-*` jobs hold the secrets, run no cargo, use only
SHA-pinned `actions/checkout`, `download-artifact` and `upload-artifact`, and
have `permissions: contents: read`. The `unsigned-*` artifacts are not
published. Order on Windows is exe -> installer (+ uninstaller) -> ZIP.

Secrets are never echoed: they reach steps through `env:` only, the decoded
`.pfx` / `.p12` live under `$RUNNER_TEMP` at fixed paths, and `always()` steps
delete them (and the temporary keychain) even when a step fails. The password
is visible on signtool's command line (`/p`; a file-based PFX has no
alternative) and on `notarytool store-credentials --password`; both are masked
secrets in an isolated job. The macOS keychain auto-locks after 15 minutes.
If only one secret of a pair is set the workflow emits a `::warning::` and
builds unsigned (or ad-hoc) rather than failing.

## Dry run

Run the workflow manually: Actions > Release > Run workflow (or
`gh workflow run release.yml --ref <branch>`), `dry_run` left true. It uses the
workspace version, skips the tag/CHANGELOG check and the tests, builds, signs,
notarizes (when the secrets exist), smoke-tests, and uploads the signed
packages as workflow artifacts (`signex-<target>-<version>`). It never creates
a Release. Download the artifacts and run the verify commands below. Do this
before the first tagged release with real secrets.

## Checksums and SBOM

The `release` job publishes `SHA256SUMS.txt`, a `sha256sum *` over every asset
(installers, packages and the SBOM). It is an integrity check, not a
signature: it proves the download matches what the workflow produced, not who
produced it.

The `sbom` job writes one CycloneDX 1.5 JSON SBOM per release,
`signex-<version>.cdx.json`, and the `release` job attaches and checksums it
like any other asset. Design choices:

- One workspace-wide file, not one per platform: the `signex` binary's
  resolved dependency graph is generated with `cargo cyclonedx --describe
  binaries --target all`, so platform-gated crates of every target (Windows,
  macOS, Linux) appear once instead of four near-identical files.
- It lists the `signex-*` workspace crates and every resolved third-party
  crate (build dependencies included) with versions and SPDX licence
  expressions, the same data `deny.toml` checks.
- The generator is pinned to an exact `cargo-cyclonedx` version (the
  `CARGO_CYCLONEDX_VERSION` env var in `release.yml`) and installed with
  `--locked`. It runs in its own job with `permissions: contents: read` and no
  secrets; the Release upload stays in the `release` job.
- It also runs on a dry run: download the `signex-sbom-<version>` artifact
  from the run page to inspect it.
- It is not a merge gate and carries no signature or build attestation.

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
  installer and the portable ZIP are built, so both carry the signed exe).
  ISCC then signs the installer and the uninstaller stub itself through the
  `SignTool=signex` / `SignedUninstaller=yes` directives in `signex.iss`
  (defined only when a certificate exists). The final installer is checked
  with `signtool verify /pa`; the uninstaller is only exercised after install
  (`signtool verify /pa` on the installed `unins000.exe`). signtool comes from
  the runner's Windows SDK.
- macOS (`installer/macos/build-dmg.sh` + workflow): the `.p12` and Apple's
  Developer ID G2 intermediate (`installer/macos/DeveloperIDG2CA.cer`, from
  apple.com/certificateauthority, expires 2031-09-17) are imported into a
  temporary keychain; the executable and then the `.app` are signed with
  `--options runtime --timestamp` (hardened runtime; inside-out, no `--deep`);
  the DMG is built and signed (codesign calls retry 3 times); the Apple
  credentials are stored in the temporary keychain with `notarytool
  store-credentials` and `notarytool submit --keychain-profile --wait
  --timeout 30m --output-format json` notarizes the DMG. Anything other than
  status `Accepted` prints `notarytool log` and fails the job; otherwise
  `xcrun stapler staple` attaches the ticket. No entitlements are used
  (no JIT or unsigned plugins; wgpu/Metal needs no exception). If a future
  feature needs one, set `MACOS_ENTITLEMENTS` for `build-dmg.sh`.

## Verifying a release artifact

```
signtool verify /pa /v signex-setup-x86_64-<v>.exe     # Windows (also signex.exe in the ZIP)
codesign --verify --deep --strict --verbose=2 /Applications/Signex.app
spctl -a -vv /Applications/Signex.app                  # expect: accepted, source=Notarized Developer ID
spctl -a -vv -t open --context context:primary-signature signex-macos-universal-<v>.dmg
xcrun stapler validate signex-macos-universal-<v>.dmg
```

## Not verified at authoring time

The signing path has not run against real certificates. Use the dry run above
first; a failed leg on a real tag means no GitHub Release for that tag, and
tags cannot be re-used. Specifically untested: signtool discovery on the
runner, ISCC's `/Ssignex` quoting with `pwsh`, the keychain import, and
notarytool's JSON output.
