#!/bin/sh
# Install the prebuilt `anikit-video` binary (no Rust toolchain, no compiling).
#   curl -fsSL https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.sh | sh
# Options (environment): ANIKIT_INSTALL_DIR (default ~/.local/bin), ANIKIT_VERSION (release tag, default video-latest),
# ANIKIT_BASE_URL (a mirror holding the release assets).
set -eu

repo="knihalani/anikit"
tag="${ANIKIT_VERSION:-video-latest}"
dir="${ANIKIT_INSTALL_DIR:-$HOME/.local/bin}"

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  MINGW* | MSYS* | CYGWIN*) echo "On Windows use install.ps1 (PowerShell)." >&2; exit 1 ;;
  *) echo "anikit-video: no prebuilt binary for $os; build from source: cargo install --git https://github.com/$repo anikit-video (about 4 min)" >&2; exit 1 ;;
esac
case "$arch" in
  x86_64 | amd64) arch=x64 ;;
  aarch64 | arm64) arch=arm64 ;;
  *) echo "anikit-video: no prebuilt binary for $arch" >&2; exit 1 ;;
esac

asset="anikit-video-$os-$arch.tar.gz"
url="${ANIKIT_BASE_URL:-https://github.com/$repo/releases/download/$tag}/$asset"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL --retry 3 "$1" -o "$2"; else wget -q "$1" -O "$2"; fi
}
if ! fetch "$url" "$tmp/$asset"; then
  echo "anikit-video: no prebuilt $os-$arch binary in release '$tag' yet." >&2
  echo "Build it instead (about 4 min): cargo install --git https://github.com/$repo anikit-video" >&2
  exit 1
fi
tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$dir"
mv "$tmp/anikit-video" "$dir/anikit-video"
chmod +x "$dir/anikit-video"

echo "installed $dir/anikit-video"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "add it to your PATH:  export PATH=\"$dir:\$PATH\"" ;;
esac
command -v ffmpeg >/dev/null 2>&1 || echo "note: MP4/PNG/GIF render without ffmpeg; audio, video layers and .webm/.mov need ffmpeg on PATH"
