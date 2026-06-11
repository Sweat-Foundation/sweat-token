#!/bin/bash
set -eox pipefail

rustup component add clippy

cargo clippy --all --target wasm32-unknown-unknown \
  -- \
  -D warnings
