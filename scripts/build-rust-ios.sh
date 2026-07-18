#!/usr/bin/env bash

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root/core"

rustup target add aarch64-apple-ios-sim
IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}" cargo build \
  --manifest-path "$repo_root/core/Cargo.toml" \
  --release \
  --target aarch64-apple-ios-sim \
  --target-dir "$repo_root/core/target"

library="$repo_root/core/target/aarch64-apple-ios-sim/release/libtts29_core.a"
test -f "$library"
echo "Built $library"
