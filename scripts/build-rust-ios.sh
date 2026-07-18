#!/usr/bin/env bash

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root/core"

targets=(aarch64-apple-ios-sim aarch64-apple-ios)
rustup target add "${targets[@]}"

for target in "${targets[@]}"; do
  IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}" cargo build \
    --manifest-path "$repo_root/core/Cargo.toml" \
    --release \
    --target "$target" \
    --target-dir "$repo_root/core/target"
done

for target in "${targets[@]}"; do
  library="$repo_root/core/target/$target/release/libtts29_core.a"
  test -f "$library"
  echo "Built $library"
done
