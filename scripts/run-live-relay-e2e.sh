#!/usr/bin/env bash

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
kokoro_env=${KOKORO_ENV_FILE:-"${HOME}/.env.tts"}
live_root=$(mktemp -d "${TMPDIR:-/tmp}/tts29-live-e2e.XXXXXX")
fixture_pid=

cleanup() {
  if [[ -n "$fixture_pid" ]]; then
    kill "$fixture_pid" 2>/dev/null || true
    wait "$fixture_pid" 2>/dev/null || true
  fi
  rm -rf -- "$live_root"
}
trap cleanup EXIT

if [[ -n "${TTS29_LIVE_AUDIO_FILE:-}" ]]; then
  if [[ ! -f "$TTS29_LIVE_AUDIO_FILE" || ! -s "$TTS29_LIVE_AUDIO_FILE" ]]; then
    echo "TTS29_LIVE_AUDIO_FILE must name a nonempty audio file" >&2
    exit 1
  fi
  fixture_ready="$live_root/kokoro-port"
  python3 "$repo_root/scripts/live-kokoro-fixture.py" \
    "$TTS29_LIVE_AUDIO_FILE" "$fixture_ready" &
  fixture_pid=$!
  for _ in {1..50}; do
    [[ -s "$fixture_ready" ]] && break
    kill -0 "$fixture_pid" 2>/dev/null || break
    sleep 0.1
  done
  if [[ ! -s "$fixture_ready" ]]; then
    echo "live Kokoro fixture did not become ready" >&2
    exit 1
  fi
  fixture_port=$(<"$fixture_ready")
  KOKORO_API_ENDPOINT="http://127.0.0.1:${fixture_port}/v1/audio/speech"
elif [[ -z "${KOKORO_API_ENDPOINT:-}" && -f "$kokoro_env" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$kokoro_env"
  set +a
fi

if [[ -z "${KOKORO_API_ENDPOINT:-}" ]]; then
  echo "KOKORO_API_ENDPOINT is required directly or through KOKORO_ENV_FILE" >&2
  exit 1
fi

if [[ -n "${KOKORO_API_KEY:-}" ]]; then
  export TTS29_KOKORO_BEARER="$KOKORO_API_KEY"
  unset TTS29_KOKORO_BASIC_USERNAME TTS29_KOKORO_BASIC_PASSWORD
elif [[ -n "${KOKORO_API_USERNAME:-}" && -n "${KOKORO_API_PASSWORD:-}" ]]; then
  export TTS29_KOKORO_BASIC_USERNAME="$KOKORO_API_USERNAME"
  export TTS29_KOKORO_BASIC_PASSWORD="$KOKORO_API_PASSWORD"
  unset TTS29_KOKORO_BEARER
else
  unset TTS29_KOKORO_BEARER TTS29_KOKORO_BASIC_USERNAME TTS29_KOKORO_BASIC_PASSWORD
fi
export TTS29_LIVE_VOICE=${TTS29_LIVE_VOICE:-${KOKORO_DEFAULT_VOICE:-af_heart}}

relay=${TTS29_LIVE_RELAY:-wss://nip29.f7z.io}
group_id=${TTS29_LIVE_GROUP:-tts29-live-$(date -u +%Y%m%d%H%M%S)}
blossom=${TTS29_LIVE_BLOSSOM:-https://blossom.primal.net}
export TTS29_LIVE_CREATE_GROUP=${TTS29_LIVE_CREATE_GROUP:-1}
config_path="$live_root/daemon.json"

jq -n \
  --arg socket "$live_root/runtime/daemon.sock" \
  --arg journal "$live_root/jobs" \
  --arg work "$live_root/work" \
  --arg store "$live_root/nmp" \
  --arg host "$relay" \
  --arg group "$group_id" \
  --arg identity "$live_root/daemon.key" \
  --arg kokoro "$KOKORO_API_ENDPOINT" \
  --arg blossom "$blossom" \
  '{
    socket_path: $socket,
    journal_root: $journal,
    work_root: $work,
    nmp_store_path: $store,
    daemon_identity_path: $identity,
    host: $host,
    group_id: $group,
    owner_pubkey: ("1" * 64),
    kokoro: {
      endpoint: $kokoro,
      request_timeout_seconds: 120,
      max_audio_bytes: 52428800,
      allow_insecure_loopback: true
    },
    blossom: {
      server: $blossom,
      request_timeout_seconds: 60,
      authorization_lifetime_seconds: 300,
      max_upload_bytes: 52428800
    },
    receipt_timeout_seconds: 60
  }' >"$config_path"
chmod 600 "$config_path"

live_target=${TTS29_LIVE_TARGET_DIR:-/Volumes/BuildCache/tts29-live-e2e}
CARGO_TARGET_DIR="$live_target" cargo run --quiet \
  --manifest-path "$repo_root/daemon/Cargo.toml" \
  --bin tts29-live-e2e -- \
  --config "$config_path"
