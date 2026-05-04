#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
    printf 'Usage: %s input-dir output.swu\n' "$0" >&2
    exit 1
fi

indir=$1
output=$2

python3 - "$indir" "$output" <<'PY'
import json
import hashlib
import pathlib
import stat
import sys


HEADER_LEN = 110
MANIFEST = ".swu-manifest.json"


def align4_len(value):
    return (-value) % 4


def fail(message):
    print(message, file=sys.stderr)
    sys.exit(1)


def safe_path(root, name):
    if name.startswith("/"):
        fail(f"absolute archive path is not supported: {name}")
    path = (root / name).resolve()
    root_resolved = root.resolve()
    if path != root_resolved and root_resolved not in path.parents:
        fail(f"archive path escapes input directory: {name}")
    return path


def checksum(payload):
    return sum(payload) & 0xFFFFFFFF


def header_for(item, payload):
    name_bytes = item["name"].encode("utf-8") + b"\0"
    values = [
        item["ino"],
        item["mode"],
        item["uid"],
        item["gid"],
        item["nlink"],
        item["mtime"],
        len(payload),
        item["devmajor"],
        item["devminor"],
        item["rdevmajor"],
        item["rdevminor"],
        len(name_bytes),
        checksum(payload),
    ]
    return b"070702" + b"".join(f"{value & 0xFFFFFFFF:08X}".encode("ascii") for value in values)


def append_entry(out, item, payload):
    name_bytes = item["name"].encode("utf-8") + b"\0"
    out.extend(header_for(item, payload))
    if len(out) % 4 != 2:  # 110-byte newc/crc header always ends at +2 mod 4.
        fail("internal cpio alignment error before filename")
    out.extend(name_bytes)
    out.extend(b"\0" * align4_len(len(out)))
    out.extend(payload)
    out.extend(b"\0" * align4_len(len(out)))


if len(sys.argv) != 3:
    fail("Usage: build-swu.sh input-dir output.swu")

root = pathlib.Path(sys.argv[1])
output = pathlib.Path(sys.argv[2])
manifest_path = root / MANIFEST

if not root.is_dir():
    fail(f"input directory does not exist: {root}")
if not manifest_path.is_file():
    fail(f"missing manifest: {manifest_path}")

manifest = json.loads(manifest_path.read_text())
if manifest.get("format") != "svr4-crc-cpio":
    fail("manifest was not created from a CRC SWU archive")

entry_names = [item["name"] for item in manifest["entries"]]
if "cpio_item_md5" in entry_names:
    lines = []
    for item in manifest["entries"]:
        name = item["name"]
        if name == "cpio_item_md5":
            break
        path = safe_path(root, name)
        if stat.S_IFMT(item["mode"]) == stat.S_IFREG:
            lines.append(f"{hashlib.md5(path.read_bytes()).hexdigest()}  {name}\n")
    safe_path(root, "cpio_item_md5").write_text("".join(lines))

out = bytearray()
for item in manifest["entries"]:
    path = safe_path(root, item["name"])
    file_type = stat.S_IFMT(item["mode"])
    if file_type == stat.S_IFDIR:
        if not path.is_dir():
            fail(f"missing directory entry: {item['name']}")
        payload = b""
    elif file_type == stat.S_IFREG:
        if not path.is_file():
            fail(f"missing file entry: {item['name']}")
        payload = path.read_bytes()
    else:
        fail(f"unsupported cpio entry type for {item['name']}: mode {item['mode']:o}")
    append_entry(out, item, payload)

trailer = manifest["trailer"]
if trailer.get("name") != "TRAILER!!!":
    fail("manifest trailer entry is invalid")
append_entry(out, trailer, b"")
out.extend(b"\0" * int(manifest.get("final_padding", 0)))

output.parent.mkdir(parents=True, exist_ok=True)
output.write_bytes(out)
print(f"wrote {output}")
PY
