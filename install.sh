#!/bin/sh
set -eu

if [ "$(uname -s)" != "Darwin" ]; then
  echo "mico currently supports macOS only." >&2
  exit 1
fi

if [ "$(uname -m)" != "arm64" ]; then
  echo "mico v1 only ships Apple Silicon builds." >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required." >&2
  exit 1
fi

if ! command -v tar >/dev/null 2>&1; then
  echo "tar is required." >&2
  exit 1
fi

if ! command -v shasum >/dev/null 2>&1; then
  echo "shasum is required." >&2
  exit 1
fi

if ! command -v tmux >/dev/null 2>&1; then
  if command -v brew >/dev/null 2>&1; then
    brew install tmux
  else
    echo "tmux is required. Install Homebrew first so mico can install tmux for you." >&2
    exit 1
  fi
fi

REPO="${MICO_GITHUB_REPO:-thegeorgejoseph/mico}"

case "$REPO" in
  */*/*|/*|*/|"")
    echo "MICO_GITHUB_REPO must be in owner/repo format." >&2
    exit 1
    ;;
  *[!A-Za-z0-9._/-]*)
    echo "MICO_GITHUB_REPO contains invalid characters." >&2
    exit 1
    ;;
esac

OWNER="${REPO%/*}"
NAME="${REPO#*/}"

if [ -z "$OWNER" ] || [ -z "$NAME" ] || [ "$OWNER" = "$REPO" ]; then
  echo "MICO_GITHUB_REPO must be in owner/repo format." >&2
  exit 1
fi

VERSION="${MICO_VERSION:-latest}"
INSTALL_DIR="${MICO_INSTALL_DIR:-$HOME/.local/bin}"

mkdir -p "$INSTALL_DIR"

if [ "$VERSION" = "latest" ]; then
  API_URL="https://api.github.com/repos/$REPO/releases/latest"
  VERSION="$(curl -fsSL "$API_URL" | sed -n 's/.*"tag_name":[[:space:]]*"v\([^"]*\)".*/\1/p' | head -n 1)"
fi

if [ -z "$VERSION" ]; then
  echo "Could not resolve a release version for $REPO." >&2
  exit 1
fi

ARCHIVE="mico-${VERSION}-aarch64-apple-darwin.tar.gz"
CHECKSUM="${ARCHIVE}.sha256"
URL="https://github.com/$REPO/releases/download/v$VERSION/$ARCHIVE"
CHECKSUM_URL="https://github.com/$REPO/releases/download/v$VERSION/$CHECKSUM"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}

trap cleanup EXIT INT TERM

curl -fsSL "$URL" -o "$TMP_DIR/$ARCHIVE"
curl -fsSL "$CHECKSUM_URL" -o "$TMP_DIR/$CHECKSUM"

EXPECTED="$(awk '{print $1}' "$TMP_DIR/$CHECKSUM")"
ACTUAL="$(shasum -a 256 "$TMP_DIR/$ARCHIVE" | awk '{print $1}')"

if [ -z "$EXPECTED" ] || [ "$ACTUAL" != "$EXPECTED" ]; then
  echo "checksum verification failed for $ARCHIVE" >&2
  exit 1
fi

tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"
cp "$TMP_DIR/mico-$VERSION-aarch64-apple-darwin/mico" "$INSTALL_DIR/mico"
chmod +x "$INSTALL_DIR/mico"

echo "mico $VERSION installed to $INSTALL_DIR/mico"
echo "Add $INSTALL_DIR to your PATH if it is not already there."
