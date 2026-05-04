#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    printf 'Usage: %s input.zip.sig [output-dir]\n' "$0" >&2
    exit 1
fi

input=$1
outdir=${2:-release-unpacked}

if [[ ! -f "$input" ]]; then
    printf 'input does not exist: %s\n' "$input" >&2
    exit 1
fi

if [[ -e "$outdir" && -n "$(ls -A "$outdir")" ]]; then
    printf 'output directory is not empty: %s\n' "$outdir" >&2
    exit 1
fi

mkdir -p "$outdir" "$outdir/templates" "$outdir/release-signed" "$outdir/release" "$outdir/swu"

cargo build --quiet

sig_bin=target/debug/sig
base=$(basename "$input")
zip_name=${base%.sig}
zip_path="$outdir/$zip_name"

cp -p "$input" "$outdir/templates/release.zip.sig"
"$sig_bin" unpack "$input" -o "$zip_path"
unzip -q "$zip_path" -d "$outdir/release-signed"

mapfile -t signed_files < <(python3 - "$zip_path" <<'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1]) as archive:
    for item in archive.infolist():
        if not item.is_dir():
            print(item.filename)
PY
)

swu_sig_name=''
ota_sig_name=''
for name in "${signed_files[@]}"; do
    case "$name" in
        *.swu.sig) swu_sig_name=$name ;;
        ota-package-list.json.sig) ota_sig_name=$name ;;
    esac
done

if [[ -z "$swu_sig_name" || -z "$ota_sig_name" ]]; then
    printf 'release zip must contain one .swu.sig and ota-package-list.json.sig\n' >&2
    exit 1
fi

swu_name=${swu_sig_name%.sig}
ota_name=${ota_sig_name%.sig}

cp -p "$outdir/release-signed/$swu_sig_name" "$outdir/templates/$swu_sig_name"
cp -p "$outdir/release-signed/$ota_sig_name" "$outdir/templates/$ota_sig_name"

"$sig_bin" unpack "$outdir/release-signed/$swu_sig_name" -o "$outdir/release/$swu_name"
"$sig_bin" unpack "$outdir/release-signed/$ota_sig_name" -o "$outdir/release/$ota_name"
scripts/extract-swu.sh "$outdir/release/$swu_name" "$outdir/swu"

cat > "$outdir/repack.env" <<EOF
ORIGINAL_SIG=$input
ZIP_NAME=$zip_name
SWU_SIG_NAME=$swu_sig_name
SWU_NAME=$swu_name
OTA_SIG_NAME=$ota_sig_name
OTA_NAME=$ota_name
EOF

printf 'unpacked %s to %s\n' "$input" "$outdir"
