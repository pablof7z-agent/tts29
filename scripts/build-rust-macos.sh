#!/usr/bin/env bash

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)

MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.0}" cargo build \
  --manifest-path "$repo_root/core/Cargo.toml" \
  --release \
  --target-dir "$repo_root/core/target"

library="$repo_root/core/target/release/libtts29_core.a"
test -f "$library"
echo "Built $library"
