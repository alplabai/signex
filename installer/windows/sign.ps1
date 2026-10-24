# Authenticode-sign one or more files with the runner's SDK signtool.
#
#   installer/windows/sign.ps1 <file> [<file>...]
#
# Reads (all set by the "Prepare Windows signing" step in release.yml):
#   SIGNTOOL_PATH  - full path to signtool.exe
#   SIGN_PFX_PATH  - decoded .pfx on disk (deleted by an always() step)
#   WINDOWS_CERT_PASSWORD - .pfx password (a masked secret; never echoed)
#
# SHA-256 file digest + RFC 3161 SHA-256 timestamp, then `signtool verify /pa`.
# Timestamp servers flake, so each file retries a few times.
param([Parameter(Mandatory, ValueFromRemainingArguments)][string[]]$Files)

$ErrorActionPreference = 'Stop'
$timestampUrl = 'http://timestamp.digicert.com'

foreach ($f in $Files) {
    $signed = $false
    for ($i = 1; $i -le 3 -and -not $signed; $i++) {
        & $env:SIGNTOOL_PATH sign /f $env:SIGN_PFX_PATH /p $env:WINDOWS_CERT_PASSWORD `
            /fd SHA256 /tr $timestampUrl /td SHA256 $f
        if ($LASTEXITCODE -eq 0) { $signed = $true } else { Start-Sleep -Seconds (5 * $i) }
    }
    if (-not $signed) { throw "signtool sign failed for $f" }
    & $env:SIGNTOOL_PATH verify /pa /v $f
    if ($LASTEXITCODE -ne 0) { throw "signtool verify /pa failed for $f" }
}
