#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <stage_dir> <version>" >&2
  exit 1
fi

STAGE_DIR="$1"
VERSION="$2"
ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
ELECTRON_APP="${ROOT_DIR}/app/node_modules/electron/dist/Electron.app"
APP_BUNDLE="${STAGE_DIR}/mico.app"
PLIST_PATH="${APP_BUNDLE}/Contents/Info.plist"
EXECUTABLE_PATH="${APP_BUNDLE}/Contents/MacOS"
RESOURCES_PATH="${APP_BUNDLE}/Contents/Resources"
APP_RESOURCES_PATH="${RESOURCES_PATH}/app"
BACKEND_SOURCE="${STAGE_DIR}/backend/mico-desktop"
RENDERER_SOURCE="${STAGE_DIR}/renderer"
APP_SOURCE="${STAGE_DIR}/app"
ELECTRON_SOURCE="${APP_SOURCE}/electron"
ICON_SOURCE="${APP_SOURCE}/assets/mico-shell-icon.icns"

if [ ! -d "${ELECTRON_APP}" ]; then
  echo "missing Electron runtime at ${ELECTRON_APP}. Run make install first." >&2
  exit 1
fi

if [ ! -x "${BACKEND_SOURCE}" ]; then
  echo "missing backend binary at ${BACKEND_SOURCE}" >&2
  exit 1
fi

if [ ! -d "${RENDERER_SOURCE}" ]; then
  echo "missing renderer build at ${RENDERER_SOURCE}" >&2
  exit 1
fi

if [ ! -f "${APP_SOURCE}/main.js" ] || [ ! -f "${APP_SOURCE}/preload.js" ] || [ ! -f "${APP_SOURCE}/package.json" ] || [ ! -d "${ELECTRON_SOURCE}" ]; then
  echo "missing staged Electron app files in ${APP_SOURCE}" >&2
  exit 1
fi

if [ ! -f "${ICON_SOURCE}" ]; then
  echo "missing icon file at ${ICON_SOURCE}" >&2
  exit 1
fi

set_plist_value() {
  key="$1"
  type="$2"
  value="$3"

  if /usr/libexec/PlistBuddy -c "Set :${key} ${value}" "${PLIST_PATH}" >/dev/null 2>&1; then
    return 0
  fi

  /usr/libexec/PlistBuddy -c "Add :${key} ${type} ${value}" "${PLIST_PATH}" >/dev/null
}

delete_plist_value() {
  key="$1"
  /usr/libexec/PlistBuddy -c "Delete :${key}" "${PLIST_PATH}" >/dev/null 2>&1 || true
}

echo "==> packaging unsigned mico.app"
rm -rf "${APP_BUNDLE}"
ditto "${ELECTRON_APP}" "${APP_BUNDLE}"

rm -f "${RESOURCES_PATH}/default_app.asar"
rm -rf "${APP_RESOURCES_PATH}" "${RESOURCES_PATH}/backend"
mkdir -p "${APP_RESOURCES_PATH}" "${RESOURCES_PATH}/backend"

cp -R "${RENDERER_SOURCE}" "${APP_RESOURCES_PATH}/dist"
cp -R "${APP_SOURCE}/assets" "${APP_RESOURCES_PATH}/assets"
cp -R "${ELECTRON_SOURCE}" "${APP_RESOURCES_PATH}/electron"
cp "${APP_SOURCE}/main.js" "${APP_RESOURCES_PATH}/main.js"
cp "${APP_SOURCE}/preload.js" "${APP_RESOURCES_PATH}/preload.js"
cp "${APP_SOURCE}/package.json" "${APP_RESOURCES_PATH}/package.json"
cp "${BACKEND_SOURCE}" "${RESOURCES_PATH}/backend/mico-desktop"
chmod +x "${RESOURCES_PATH}/backend/mico-desktop"

if [ -f "${EXECUTABLE_PATH}/Electron" ]; then
  mv "${EXECUTABLE_PATH}/Electron" "${EXECUTABLE_PATH}/mico"
fi

cp "${ICON_SOURCE}" "${RESOURCES_PATH}/mico.icns"

set_plist_value "CFBundleDisplayName" string "mico"
set_plist_value "CFBundleName" string "mico"
set_plist_value "CFBundleExecutable" string "mico"
set_plist_value "CFBundleIconFile" string "mico.icns"
set_plist_value "CFBundleIdentifier" string "com.thegeorgejoseph.mico"
set_plist_value "CFBundleShortVersionString" string "${VERSION}"
set_plist_value "CFBundleVersion" string "${VERSION}"
delete_plist_value "ElectronAsarIntegrity"

node -e '
const fs = require("node:fs");
const file = process.argv[1];
const version = process.argv[2];
const pkg = JSON.parse(fs.readFileSync(file, "utf8"));
pkg.version = version;
fs.writeFileSync(file, `${JSON.stringify(pkg, null, 2)}\n`);
' "${APP_RESOURCES_PATH}/package.json" "${VERSION}"

echo "packaged ${APP_BUNDLE}"
