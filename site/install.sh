#!/usr/bin/env sh
set -eu

REPO="ESAK9781/grex"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

echo "Fetching latest Grex release from https://github.com/$REPO/releases ..."
API_URL="https://api.github.com/repos/$REPO/releases/latest"
RELEASE_JSON="$(curl -fsSL -H 'User-Agent: grex-installer' "$API_URL")"

case "$OS" in
  darwin)
    PATTERN='\.dmg"'
    ;;
  linux)
    PATTERN='\.AppImage"'
    ;;
  *)
    echo "Unsupported OS: $OS (arch: $ARCH). Download manually from https://github.com/$REPO/releases"
    exit 1
    ;;
esac

ASSET_URL="$(printf '%s\n' "$RELEASE_JSON" | grep -oE '"browser_download_url": *"[^"]+"' | grep -E "$PATTERN" | head -n1 | cut -d'"' -f4 || true)"

if [ -z "$ASSET_URL" ]; then
  echo "No prebuilt $OS asset found on the latest release yet."
  echo "Build from source with:"
  echo "  git clone https://github.com/$REPO.git && cd grex && npm install && npm run tauri build"
  exit 1
fi

OUT_FILE="/tmp/$(basename "$ASSET_URL")"
echo "Downloading $ASSET_URL -> $OUT_FILE"
curl -fL "$ASSET_URL" -o "$OUT_FILE"

if [ "$OS" = "linux" ]; then
  chmod +x "$OUT_FILE"
  mkdir -p "$HOME/.local/bin"
  mv "$OUT_FILE" "$HOME/.local/bin/grex"
  echo "Installed Grex to $HOME/.local/bin/grex"
else
  echo "Opening installer: $OUT_FILE"
  open "$OUT_FILE"
fi
