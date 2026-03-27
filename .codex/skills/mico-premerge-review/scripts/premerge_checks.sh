#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

cd "${REPO_ROOT}"

echo "==> cargo fmt --check"
cargo fmt --check

echo "==> cargo check --workspace --all-targets"
cargo check --workspace --all-targets

echo "==> cargo test --workspace --all-targets -q"
cargo test --workspace --all-targets -q

echo "==> cargo clippy -q"
cargo clippy -q
