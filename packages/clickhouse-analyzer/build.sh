#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "==> Building WASM..."
# wasm-pack --out-dir is relative to the crate root, so build to default
# location then move into this package directory.
wasm-pack build "$REPO_ROOT" --target web --features wasm,serde
rm -rf "$SCRIPT_DIR/pkg"
mv "$REPO_ROOT/pkg" "$SCRIPT_DIR/pkg"

echo "==> Compiling TypeScript..."
cd "$SCRIPT_DIR"
npx tsc

echo "==> Done. Output in dist/ and pkg/"
echo ""
echo "WASM binary size:"
ls -lh pkg/clickhouse_analyzer_bg.wasm
echo ""
echo "Gzipped size:"
gzip -c pkg/clickhouse_analyzer_bg.wasm | wc -c | awk '{printf "%.1f KB\n", $1/1024}'
