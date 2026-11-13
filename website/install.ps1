# One-command verified installer for YouTFast on Windows (x86_64).
# Usage:
#   irm https://youtfast.pages.dev/install.ps1 | iex
# Optional environment variables:
#   $env:YOUTFAST_VERSION = '0.6.2'   # install a specific version
#   $env:YOUTFAST_SILENT  = '1'       # run silent per-user installation

$ErrorActionPreference = 'Stop'

if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'YouTFast requires 64-bit Windows (x86_64).'
}

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo = 'bcosta19/youtfast'
$Version = if ($env:YOUTFAST_VERSION) { $env:YOUTFAST_VERSION.Trim().TrimStart('v') } else { '' }

if (-not $Version) {
    Write-Host 'Checking latest YouTFast release...'
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ 'User-Agent' = 'youtfast-installer' }
    $Version = ($Release.tag_name -replace '^v', '').Trim()
}

if ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw "Invalid YouTFast version '$Version' (expected MAJOR.MINOR.PATCH)."
}

$BaseUrl = "https://github.com/$Repo/releases/download/v$Version"
$Asset = "youtfast-v$Version-x86_64-windows-setup.exe"
$WorkDir = Join-Path ([IO.Path]::GetTempPath()) ("youtfast-install-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null

try {
    $ChecksumsPath = Join-Path $WorkDir 'checksums.txt'
    $SetupPath = Join-Path $WorkDir $Asset

    Write-Host "Downloading $Asset and checksums.txt..."
    Invoke-WebRequest -Uri "$BaseUrl/checksums.txt" -OutFile $ChecksumsPath -UseBasicParsing
    Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $SetupPath -UseBasicParsing

    $Expected = $null
    foreach ($Line in (Get-Content -Path $ChecksumsPath)) {
        if ($Line -match '^\s*([0-9a-fA-F]{64})\s+\*?(.+?)\s*$') {
            if ($Matches[2] -eq $Asset) {
                $Expected = $Matches[1].ToLowerInvariant()
                break
            }
        }
    }

    if (-not $Expected) {
        throw "Checksum entry for '$Asset' was not found in checksums.txt."
    }

    $Actual = (Get-FileHash -Path $SetupPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Actual -ne $Expected) {
        throw "SHA-256 verification failed for $Asset (expected $Expected, got $Actual)."
    }
    Write-Host "Verified SHA-256 checksum for $Asset."

    $ArgsList = @()
    if ($env:YOUTFAST_SILENT -eq '1') {
        $ArgsList += '/S'
    }

    $Process = if ($ArgsList.Count -gt 0) {
        Start-Process -FilePath $SetupPath -ArgumentList $ArgsList -Wait -PassThru
    } else {
        Start-Process -FilePath $SetupPath -Wait -PassThru
    }

    if ($Process.ExitCode -ne 0) {
        throw "Installer exited with code $($Process.ExitCode)."
    }
}
finally {
    if (Test-Path $WorkDir) {
        Remove-Item -Path $WorkDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
