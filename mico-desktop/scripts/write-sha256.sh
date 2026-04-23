#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <file>" >&2
  exit 1
fi

TARGET="$1"
CHECKSUM_PATH="${TARGET}.sha256"

if [ ! -f "${TARGET}" ]; then
  echo "missing file at ${TARGET}" >&2
  exit 1
fi

shasum -a 256 "${TARGET}" | awk '{print $1}' > "${CHECKSUM_PATH}"
echo "wrote ${CHECKSUM_PATH}"
