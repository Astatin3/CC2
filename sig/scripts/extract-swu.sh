#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
    printf 'Usage: %s input.swu output-dir\n' "$0" >&2
    exit 1
fi

input=$1
outdir=$2

python3 - "$input" "$outdir" <<'PY'
import json
import os
import pathlib
import stat
import sys


HEADER_LEN = 110
MANIFEST = ".swu-manifest.json"


def align4(value):
    return (value + 3) & ~3


def fail(message):
    print(message, file=sys.stderr)
    sys.exit(1)


def safe_path(root, name):
    if name.startswith("/"):
        fail(f"absolute archive path is not supported: {name}")
    path = (root / name).resolve()
    root_resolved = root.resolve()
    if path != root_resolved and root_resolved not in path.parents:
        fail(f"archive path escapes output directory: {name}")
    return path


if len(sys.argv) != 3:
    fail("Usage: extract-swu.sh input.swu output-dir")

input_path = pathlib.Path(sys.argv[1])
out_root = pathlib.Path(sys.argv[2])

if not input_path.is_file():
    fail(f"input does not exist: {input_path}")

if out_root.exists() and any(out_root.iterdir()):
    fail(f"output directory is not empty: {out_root}")
out_root.mkdir(parents=True, exist_ok=True)

data = input_path.read_bytes()
offset = 0
entries = []
trailer = None

while True:
    if offset + HEADER_LEN > len(data):
        fail("truncated cpio header")

    header_offset = offset
    header = data[offset:offset + HEADER_LEN]
    offset += HEADER_LEN
    fields = [header[i:i + 8].decode("ascii") for i in range(6, HEADER_LEN, 8)]
    magic = header[:6].decode("ascii")
    if magic != "070702":
        fail(f"unsupported cpio magic at offset {header_offset}: {magic!r}")

    values = [int(field, 16) for field in fields]
    (
        ino,
        mode,
        uid,
        gid,
        nlink,
        mtime,
        filesize,
        devmajor,
        devminor,
        rdevmajor,
        rdevminor,
        namesize,
        check,
    ) = values

    if offset + namesize > len(data):
        fail("truncated cpio filename")
    raw_name = data[offset:offset + namesize]
    offset += namesize
    if not raw_name.endswith(b"\0"):
        fail("cpio filename is missing NUL terminator")
    name = raw_name[:-1].decode("utf-8")
    offset = align4(offset)

    if offset + filesize > len(data):
        fail(f"truncated cpio payload for {name}")
    payload = data[offset:offset + filesize]
    offset += filesize
    offset = align4(offset)

    item = {
        "name": name,
        "ino": ino,
        "mode": mode,
        "uid": uid,
        "gid": gid,
        "nlink": nlink,
        "mtime": mtime,
        "devmajor": devmajor,
        "devminor": devminor,
        "rdevmajor": rdevmajor,
        "rdevminor": rdevminor,
    }

    if name == "TRAILER!!!":
        trailer = item
        break

    entries.append(item)
    path = safe_path(out_root, name)
    file_type = stat.S_IFMT(mode)
    if file_type == stat.S_IFDIR:
        path.mkdir(parents=True, exist_ok=True)
    elif file_type == stat.S_IFREG:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
    else:
        fail(f"unsupported cpio entry type for {name}: mode {mode:o}")

    os.chmod(path, stat.S_IMODE(mode))
    os.utime(path, (mtime, mtime), follow_symlinks=False)

if trailer is None:
    fail("archive has no TRAILER!!! entry")

final_padding = len(data) - offset
if data[offset:] != b"\0" * final_padding:
    fail("archive has non-zero bytes after TRAILER!!!")

manifest = {
    "format": "svr4-crc-cpio",
    "block_size": 512,
    "final_padding": final_padding,
    "entries": entries,
    "trailer": trailer,
}
(out_root / MANIFEST).write_text(json.dumps(manifest, indent=2) + "\n")
print(f"extracted {len(entries)} entries to {out_root}")
PY
