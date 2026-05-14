#!/bin/bash
set -eox pipefail

echo ">> Building contract"

cargo near build non-reproducible-wasm \
    --manifest-path contract/Cargo.toml \
    --out-dir res
