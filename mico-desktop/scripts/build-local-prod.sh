#!/bin/sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP_DIR="${ROOT_DIR}/app"
BACKEND_DIR="${ROOT_DIR}/backend"
OUTPUT_DIR="${ROOT_DIR}/dist/local-prod"
VERSION="$(node -p "require('${APP_DIR}/package.json').version")"

echo "==> building backend binary"
mkdir -p "${BACKEND_DIR}/bin"
(cd "${BACKEND_DIR}" && go build -o ./bin/mico-desktop ./cmd/mico-desktop)

echo "==> building renderer"
(cd "${APP_DIR}" && npm run build)

echo "==> staging local production assets"
rm -rf "${OUTPUT_DIR}"
mkdir -p "${OUTPUT_DIR}/backend" "${OUTPUT_DIR}/renderer" "${OUTPUT_DIR}/app"
cp "${BACKEND_DIR}/bin/mico-desktop" "${OUTPUT_DIR}/backend/mico-desktop"
cp -R "${APP_DIR}/dist/." "${OUTPUT_DIR}/renderer/"
cp -R "${APP_DIR}/assets" "${OUTPUT_DIR}/app/assets"
cp -R "${APP_DIR}/electron" "${OUTPUT_DIR}/app/electron"
cp "${APP_DIR}/main.js" "${OUTPUT_DIR}/app/main.js"
cp "${APP_DIR}/preload.js" "${OUTPUT_DIR}/app/preload.js"
cp "${APP_DIR}/package.json" "${OUTPUT_DIR}/app/package.json"

"${ROOT_DIR}/scripts/package-macos-app.sh" "${OUTPUT_DIR}" "${VERSION}"

cat > "${OUTPUT_DIR}/manifest.txt" <<EOF
version=${VERSION}
backend_binary=${OUTPUT_DIR}/backend/mico-desktop
renderer_dir=${OUTPUT_DIR}/renderer
app_bundle=${OUTPUT_DIR}/mico.app
electron_entry=${OUTPUT_DIR}/mico.app/Contents/Resources/app/main.js
mode=local-production-smoke-test
EOF

echo "staged local production assets in ${OUTPUT_DIR}"
