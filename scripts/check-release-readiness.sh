#!/usr/bin/env bash

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
source_only=false
failures=0

usage() {
  cat <<'EOF'
Usage: scripts/check-release-readiness.sh [--source-only]

--source-only  Validate committed product identity and compliance metadata.

Without --source-only, also require a clean tagged commit, a UTC build number,
local manual signing material, and App Store Connect API metadata. Private key
and provisioning-profile contents are never printed.
EOF
}

pass() {
  printf 'PASS  %s\n' "$1"
}

fail() {
  printf 'FAIL  %s\n' "$1" >&2
  failures=$((failures + 1))
}

read_setting() {
  local file=$1
  local key=$2
  awk -F '=' -v key="$key" '
    $1 ~ "^[[:space:]]*" key "[[:space:]]*$" {
      value = $2
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
      print value
      exit
    }
  ' "$file"
}

check_equal() {
  local actual=$1
  local expected=$2
  local label=$3
  if [[ "$actual" == "$expected" ]]; then
    pass "$label is $expected"
  else
    fail "$label must be $expected (found ${actual:-missing})"
  fi
}

has_app_store_profile() {
  local profile_dir=$1
  local bundle_id=$2
  local profile
  local profile_plist
  local application_identifier
  local get_task_allow

  while IFS= read -r -d '' profile; do
    profile_plist=$(mktemp)
    if security cms -D -i "$profile" >"$profile_plist" 2>/dev/null; then
      application_identifier=$(
        /usr/libexec/PlistBuddy \
          -c 'Print :Entitlements:application-identifier' \
          "$profile_plist" 2>/dev/null || true
      )
      get_task_allow=$(
        /usr/libexec/PlistBuddy \
          -c 'Print :Entitlements:get-task-allow' \
          "$profile_plist" 2>/dev/null || true
      )
      if [[ "$application_identifier" == *."$bundle_id" ]] \
        && [[ "$get_task_allow" == "false" ]] \
        && ! /usr/libexec/PlistBuddy \
          -c 'Print :ProvisionedDevices' "$profile_plist" >/dev/null 2>&1; then
        rm -f "$profile_plist"
        return 0
      fi
    fi
    rm -f "$profile_plist"
  done < <(find "$profile_dir" -maxdepth 1 -type f -name '*.mobileprovision' -print0)

  return 1
}

while (($#)); do
  case "$1" in
    --source-only)
      source_only=true
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
  shift
done

ios_config="$repo_root/apple/Config/Shared.xcconfig"
mac_config="$repo_root/apple/TTS29Mac/Config/Shared.xcconfig"

ios_version=$(read_setting "$ios_config" MARKETING_VERSION)
mac_version=$(read_setting "$mac_config" MARKETING_VERSION)
ios_bundle_id=$(read_setting "$ios_config" PRODUCT_BUNDLE_IDENTIFIER)
mac_bundle_id=$(read_setting "$mac_config" PRODUCT_BUNDLE_IDENTIFIER)
ios_team=$(read_setting "$ios_config" DEVELOPMENT_TEAM)
mac_team=$(read_setting "$mac_config" DEVELOPMENT_TEAM)

check_equal "$ios_version" "$mac_version" "Apple marketing versions"
check_equal "$ios_bundle_id" "com.pablof7z.tts29" "iOS bundle identifier"
check_equal "$mac_bundle_id" "com.pablof7z.tts29.macos" "macOS bundle identifier"
check_equal "$ios_team" "456SHKPP26" "iOS development team"
check_equal "$mac_team" "456SHKPP26" "macOS development team"

for config in "$ios_config" "$mac_config"; do
  if grep -Eq '^INFOPLIST_KEY_ITSAppUsesNonExemptEncryption[[:space:]]*=[[:space:]]*NO$' "$config"; then
    pass "${config#"$repo_root/"} declares no non-exempt encryption"
  else
    fail "$config must declare ITSAppUsesNonExemptEncryption = NO"
  fi
done

nmp_revisions=$(
  grep -E '^(nmp|nmp-nip29)[[:space:]]*=' "$repo_root/core/Cargo.toml" \
    | grep -Eo 'rev[[:space:]]*=[[:space:]]*"[0-9a-f]{40}"' \
    | cut -d '"' -f 2 || true
)
nmp_revision_count=$(printf '%s\n' "$nmp_revisions" | grep -Ec '^[0-9a-f]{40}$' || true)
nmp_unique_count=$(printf '%s\n' "$nmp_revisions" | sort -u | grep -Ec '^[0-9a-f]{40}$' || true)
if [[ "$nmp_revision_count" == 2 && "$nmp_unique_count" == 1 ]]; then
  pass "NMP dependencies use one exact public revision"
else
  fail "both NMP dependencies must use the same exact 40-character revision"
fi

if [[ "$source_only" == true ]]; then
  if ((failures)); then
    exit 1
  fi
  pass "source release metadata is ready"
  exit 0
fi

if [[ -z "$(git -C "$repo_root" status --porcelain)" ]]; then
  pass "release worktree is clean"
else
  fail "release worktree must be clean"
fi

release_tag=${TTS29_RELEASE_TAG:-v${ios_version}}
if git -C "$repo_root" tag --points-at HEAD | grep -Fxq "$release_tag"; then
  pass "HEAD has release tag $release_tag"
else
  fail "HEAD must have release tag $release_tag"
fi

release_build=${TTS29_BUILD_NUMBER:-}
if [[ "$release_build" =~ ^[0-9]{12}$ ]]; then
  pass "UTC build number is configured"
else
  fail "TTS29_BUILD_NUMBER must be a 12-digit UTC timestamp"
fi

signing_identities=$(security find-identity -v -p codesigning 2>/dev/null || true)
if grep -q 'Apple Distribution:' <<<"$signing_identities"; then
  pass "Apple Distribution identity is installed"
else
  fail "an Apple Distribution identity is required for TestFlight"
fi
if grep -q 'Developer ID Application:' <<<"$signing_identities"; then
  pass "Developer ID Application identity is installed"
else
  fail "a Developer ID Application identity is required for macOS notarization"
fi

if [[ -n "${TTS29_PROFILE_DIR:-}" ]]; then
  profile_dirs=("$TTS29_PROFILE_DIR")
else
  profile_dirs=(
    "${HOME}/Library/MobileDevice/Provisioning Profiles"
    "${HOME}/Library/Developer/Xcode/UserData/Provisioning Profiles"
  )
fi

profile_ready=false
for profile_dir in "${profile_dirs[@]}"; do
  if [[ -d "$profile_dir" ]] && has_app_store_profile "$profile_dir" "$ios_bundle_id"; then
    profile_ready=true
    break
  fi
done
if [[ "$profile_ready" == true ]]; then
  pass "manual App Store profile matches $ios_bundle_id"
else
  fail "a local manual App Store profile is required for $ios_bundle_id"
fi

asc_key_id=${TTS29_ASC_KEY_ID:-}
asc_issuer_id=${TTS29_ASC_ISSUER_ID:-}
if [[ -n "$asc_key_id" ]]; then
  pass "App Store Connect key ID is configured"
else
  fail "TTS29_ASC_KEY_ID is required"
fi
if [[ -n "$asc_issuer_id" ]]; then
  pass "App Store Connect issuer ID is configured"
else
  fail "TTS29_ASC_ISSUER_ID is required"
fi

asc_key_path=${TTS29_ASC_KEY_PATH:-"${HOME}/.appstoreconnect/private_keys/AuthKey_${asc_key_id}.p8"}
if [[ -n "$asc_key_id" && -f "$asc_key_path" ]]; then
  pass "App Store Connect private key file is installed"
else
  fail "App Store Connect private key file is missing"
fi

if ((failures)); then
  printf '\nRelease readiness failed with %d unmet prerequisite(s).\n' "$failures" >&2
  exit 1
fi

printf '\nRelease prerequisites are present. Continue with the approved release runbook.\n'
