#!/usr/bin/env bash

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

required_lines=(
  "apple/Config/Shared.xcconfig:DEVELOPMENT_TEAM = 456SHKPP26"
  "apple/Config/Shared.xcconfig:INFOPLIST_KEY_ITSAppUsesNonExemptEncryption = NO"
  "apple/TTS29Mac/Config/Shared.xcconfig:DEVELOPMENT_TEAM = 456SHKPP26"
  "apple/TTS29Mac/Config/Shared.xcconfig:INFOPLIST_KEY_ITSAppUsesNonExemptEncryption = NO"
  "scripts/build-rust-ios.sh:aarch64-apple-ios-sim"
  "scripts/build-rust-ios.sh:aarch64-apple-ios"
  "docs/migration.md:TTS29 makes a hard cut"
)

for entry in "${required_lines[@]}"; do
  file=${entry%%:*}
  value=${entry#*:}
  rg --fixed-strings --quiet "$value" "$file" || {
    echo "missing cutover requirement in $file: $value" >&2
    exit 1
  }
done

stale_claim="must already be authorized to the NIP-29 group"
if rg --fixed-strings --quiet "$stale_claim" README.md docs apple; then
  echo "obsolete external-membership claim remains in product documentation" >&2
  exit 1
fi

echo "Standalone cutover metadata is internally consistent."
