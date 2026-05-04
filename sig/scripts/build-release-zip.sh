#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    printf 'Usage: %s input-dir [output.zip]\n' "$0" >&2
    exit 1
fi

indir=$1
output=${2:-release.zip}

python3 - "$indir" "$output" <<'PY'
import pathlib
import stat
import sys
import zipfile


def fail(message):
    print(message, file=sys.stderr)
    sys.exit(1)


if len(sys.argv) != 3:
    fail("Usage: build-release-zip.sh input-dir [output.zip]")

root = pathlib.Path(sys.argv[1])
output = pathlib.Path(sys.argv[2])

if not root.is_dir():
    fail(f"input directory does not exist: {root}")

entries = sorted(path for path in root.iterdir() if path.is_file())
if not entries:
    fail(f"input directory contains no regular files: {root}")


def zip_date_time(path):
    modified = path.stat().st_mtime
    return tuple(__import__("time").localtime(modified)[:6])

output.parent.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(output, "w") as archive:
    for src in entries:
        mode = stat.S_IMODE(src.stat().st_mode)
        info = zipfile.ZipInfo(src.name, date_time=zip_date_time(src))
        info.create_system = 3
        info.external_attr = (stat.S_IFREG | mode) << 16
        info.compress_type = zipfile.ZIP_DEFLATED
        archive.writestr(
            info,
            src.read_bytes(),
            compress_type=zipfile.ZIP_DEFLATED,
            compresslevel=6,
        )

print(f"wrote {output}")
PY
