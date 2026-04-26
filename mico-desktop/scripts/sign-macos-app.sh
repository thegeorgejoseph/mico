#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <app_bundle> <codesign_identity>" >&2
  exit 1
fi

APP_BUNDLE="$1"
CODESIGN_IDENTITY="$2"
FRAMEWORKS_DIR="${APP_BUNDLE}/Contents/Frameworks"
BACKEND_BIN="${APP_BUNDLE}/Contents/Resources/backend/mico-desktop"
ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP_ENTITLEMENTS="${ROOT_DIR}/app/electron/entitlements.mac.plist"
INHERIT_ENTITLEMENTS="${ROOT_DIR}/app/electron/entitlements.mac.inherit.plist"

if [ ! -d "${APP_BUNDLE}" ]; then
  echo "missing app bundle at ${APP_BUNDLE}" >&2
  exit 1
fi

if [ ! -d "${FRAMEWORKS_DIR}" ]; then
  echo "missing frameworks directory at ${FRAMEWORKS_DIR}" >&2
  exit 1
fi

if [ ! -f "${BACKEND_BIN}" ]; then
  echo "missing backend binary at ${BACKEND_BIN}" >&2
  exit 1
fi

if [ ! -f "${APP_ENTITLEMENTS}" ] || [ ! -f "${INHERIT_ENTITLEMENTS}" ]; then
  echo "missing macOS entitlements files under ${ROOT_DIR}/app/electron" >&2
  exit 1
fi

sign_file() {
  target="$1"
  codesign --force --sign "${CODESIGN_IDENTITY}" --timestamp "$target"
}

sign_runtime() {
  target="$1"
  codesign --force --sign "${CODESIGN_IDENTITY}" --timestamp --options runtime "$target"
}

sign_runtime_with_entitlements() {
  target="$1"
  entitlements="$2"
  codesign --force --sign "${CODESIGN_IDENTITY}" --timestamp --options runtime --entitlements "${entitlements}" "$target"
}

echo "==> signing nested binaries"

find "${FRAMEWORKS_DIR}" -type f \( -name "*.dylib" -o -name "*.so" \) -print0 | while IFS= read -r -d '' file; do
  sign_file "$file"
done

find "${FRAMEWORKS_DIR}" -type f -perm -111 ! -name "*.dylib" ! -name "*.so" -print0 | while IFS= read -r -d '' file; do
  sign_runtime "$file"
done

sign_runtime "${BACKEND_BIN}"

echo "==> signing nested bundles"

find "${FRAMEWORKS_DIR}" -depth -type d \( -name "*.app" -o -name "*.framework" -o -name "*.xpc" \) -print0 | while IFS= read -r -d '' bundle; do
  case "$bundle" in
    *.app|*.xpc)
      sign_runtime_with_entitlements "$bundle" "${INHERIT_ENTITLEMENTS}"
      ;;
    *)
      sign_runtime "$bundle"
      ;;
  esac
done

echo "==> signing app bundle"
sign_runtime_with_entitlements "${APP_BUNDLE}" "${APP_ENTITLEMENTS}"

codesign --verify --deep --strict --verbose=2 "${APP_BUNDLE}"
