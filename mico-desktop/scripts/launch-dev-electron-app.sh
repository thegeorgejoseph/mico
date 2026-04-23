#!/bin/sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP_DIR="${ROOT_DIR}/app"
DEV_APP_PATH="$("${ROOT_DIR}/scripts/sync-dev-electron-brand.sh")"
if [ ! -d "${DEV_APP_PATH}" ]; then
  echo "missing branded dev app at ${DEV_APP_PATH}" >&2
  exit 1
fi

if [ -n "${MICO_RENDERER_URL:-}" ]; then
  exec open -W -n -a "${DEV_APP_PATH}" --args "${APP_DIR}" "--mico-dev-bundle" "--mico-renderer-url=${MICO_RENDERER_URL}"
fi

exec open -W -n -a "${DEV_APP_PATH}" --args "${APP_DIR}" "--mico-dev-bundle"
