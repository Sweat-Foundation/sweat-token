#!/bin/bash
set -eox pipefail

echo ">> Building contract (integration-test feature)"

OUT_DIR="integration-tests/target/integration-wasm"
mkdir -p "$OUT_DIR"

cargo near build non-reproducible-wasm \
    --manifest-path contract/Cargo.toml \
    --features integration-test \
    --out-dir "$OUT_DIR"
