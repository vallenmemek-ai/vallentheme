#!/usr/bin/env bash
# Pemeriksaan kualitas wajib. Harus hijau sebelum merge ke main.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "== cargo fmt =="
cargo fmt --all --check

echo "== cargo clippy =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== cargo test =="
cargo test --workspace

echo
echo "SEMUA PEMERIKSAAN HIJAU"
