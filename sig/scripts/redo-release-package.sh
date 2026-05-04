#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    printf 'Usage: %s unpacked-dir [output.zip.sig]\n' "$0" >&2
    exit 1
fi

workdir=$1
output=${2:-$workdir/repacked.zip.sig}
env_file="$workdir/repack.env"

if [[ ! -f "$env_file" ]]; then
    printf 'missing repack metadata: %s\n' "$env_file" >&2
    exit 1
fi

# shellcheck disable=SC1090
source "$env_file"

required=(
    "$workdir/templates/release.zip.sig"
    "$workdir/templates/$SWU_SIG_NAME"
    "$workdir/templates/$OTA_SIG_NAME"
    "$workdir/release-signed/$SWU_SIG_NAME"
    "$workdir/release-signed/$OTA_SIG_NAME"
    "$workdir/release/$OTA_NAME"
    "$workdir/swu/.swu-manifest.json"
)

for path in "${required[@]}"; do
    if [[ ! -e "$path" ]]; then
        printf 'missing required repack input: %s\n' "$path" >&2
        exit 1
    fi
done

cargo build --quiet

sig_bin=target/debug/sig
build_dir="$workdir/build"
release_dir="$build_dir/release"
rebuilt_swu="$build_dir/$SWU_NAME"
rebuilt_ota="$build_dir/$OTA_NAME"
rebuilt_zip="$build_dir/$ZIP_NAME"

rm -rf "$build_dir"
mkdir -p "$release_dir"

scripts/build-swu.sh "$workdir/swu" "$rebuilt_swu"
"$sig_bin" repack "$rebuilt_swu" --template "$workdir/templates/$SWU_SIG_NAME" -o "$release_dir/$SWU_SIG_NAME"
cp "$workdir/release/$OTA_NAME" "$rebuilt_ota"

python3 - "$rebuilt_ota" "$workdir/release/$OTA_NAME" "$release_dir/$SWU_SIG_NAME" "$SWU_SIG_NAME" <<'PY'
import hashlib
import json
import pathlib
import sys

ota_path = pathlib.Path(sys.argv[1])
source_ota_path = pathlib.Path(sys.argv[2])
swu_sig_path = pathlib.Path(sys.argv[3])
swu_sig_name = sys.argv[4]

data = json.loads(ota_path.read_text())
digest = hashlib.sha256(swu_sig_path.read_bytes()).hexdigest()
matched = False
for package in data.get("packages", []):
    if package.get("file") == swu_sig_name:
        package["hash"] = digest
        matched = True

if not matched:
    raise SystemExit(f"ota package list does not reference {swu_sig_name}")

updated = json.dumps(data, separators=(", ", ": "))
ota_path.write_text(updated)
source_ota_path.write_text(updated)
PY

"$sig_bin" repack "$rebuilt_ota" --template "$workdir/templates/$OTA_SIG_NAME" -o "$release_dir/$OTA_SIG_NAME"

touch -r "$workdir/release-signed/$SWU_SIG_NAME" "$release_dir/$SWU_SIG_NAME"
touch -r "$workdir/release-signed/$OTA_SIG_NAME" "$release_dir/$OTA_SIG_NAME"
chmod --reference="$workdir/release-signed/$SWU_SIG_NAME" "$release_dir/$SWU_SIG_NAME"
chmod --reference="$workdir/release-signed/$OTA_SIG_NAME" "$release_dir/$OTA_SIG_NAME"

scripts/build-release-zip.sh "$release_dir" "$rebuilt_zip"
"$sig_bin" repack "$rebuilt_zip" --template "$workdir/templates/release.zip.sig" -o "$output"

printf 'rebuilt %s\n' "$output"

if [[ -n "${ORIGINAL_SIG:-}" && -f "$ORIGINAL_SIG" ]]; then
    if cmp -s "$output" "$ORIGINAL_SIG"; then
        printf 'bit-perfect match: %s\n' "$ORIGINAL_SIG"
    else
        printf 'warning: rebuilt output differs from %s\n' "$ORIGINAL_SIG" >&2
        exit 2
    fi
fi
