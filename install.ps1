# Install the prebuilt `anikit-video.exe` (no Rust toolchain, no compiling).
#   irm https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.ps1 | iex
# Options (environment): ANIKIT_INSTALL_DIR (default %LOCALAPPDATA%\anikit\bin), ANIKIT_VERSION (default video-latest).
$ErrorActionPreference = 'Stop'
$repo = 'knihalani/anikit'
$tag = if ($env:ANIKIT_VERSION) { $env:ANIKIT_VERSION } else { 'video-latest' }
$dir = if ($env:ANIKIT_INSTALL_DIR) { $env:ANIKIT_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'anikit\bin' }
$asset = 'anikit-video-windows-x64.zip'
$url = "https://github.com/$repo/releases/download/$tag/$asset"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmp, $dir | Out-Null
Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $asset) -UseBasicParsing
Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $tmp -Force
Move-Item -Force (Join-Path $tmp 'anikit-video.exe') (Join-Path $dir 'anikit-video.exe')
Remove-Item -Recurse -Force $tmp
Write-Host "installed $dir\anikit-video.exe"
if (-not (($env:PATH -split ';') -contains $dir)) {
  [Environment]::SetEnvironmentVariable('PATH', "$dir;" + [Environment]::GetEnvironmentVariable('PATH', 'User'), 'User')
  Write-Host "added $dir to your user PATH (open a new terminal)"
}
