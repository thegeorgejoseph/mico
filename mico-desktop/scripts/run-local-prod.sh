#!/bin/sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP_BUNDLE="${ROOT_DIR}/dist/local-prod/mico.app"
APP_EXECUTABLE="${APP_BUNDLE}/Contents/MacOS/mico"

"${ROOT_DIR}/scripts/build-local-prod.sh"

if [ ! -x "${APP_EXECUTABLE}" ]; then
  echo "missing packaged app executable at ${APP_EXECUTABLE}" >&2
  exit 1
fi

echo "==> launching mico in local production mode"
"${APP_EXECUTABLE}"
