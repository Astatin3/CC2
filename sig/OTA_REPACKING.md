# OTA Unpack And Repack Guide

This guide describes how to unpack and repack a Centauri Carbon 2 OTA update using
the tools in this directory.

## Build The Tool

```bash
cargo build
```

The compiled tool is:

```bash
target/debug/sig
```

## Inspect A `.sig` File

Print parsed header information:

```bash
target/debug/sig info path/to/file.sig
```

Print raw unknown header bytes too:

```bash
target/debug/sig info --raw path/to/file.sig
```

Important fields:

- `encrypted`: whether the payload is AES-CBC encrypted.
- `filesize`: plaintext payload size.
- `stored_sha256`: SHA-256 of the payload stored in the header.
- `payload_sha256_matches`: whether the stored hash matches the actual payload.
- `signature_valid`: whether the RSA signature at `0x100..0x200` verifies.

## Unpack A Single `.sig`

```bash
target/debug/sig unpack input.sig -o output-file
```

Examples:

```bash
target/debug/sig unpack ota-package-list.json.sig -o ota-package-list.json
target/debug/sig unpack cc2_eeb001_01.03.02.36_20260326171745.swu.sig -o update.swu
target/debug/sig unpack release.zip.sig -o release.zip
```

`unpack` removes the 512-byte `.sig` header. If the payload is encrypted, it also
decrypts it with the Carbon 2 AES key and trims the result to the plaintext size
stored in the header.

## Repack A Single `.sig`

Use `repack` to wrap a file in a valid signed `.sig` header:

```bash
target/debug/sig repack input-file -o output.sig
```

For encrypted inner files, use a template from the original package:

```bash
target/debug/sig repack modified.swu \
  --template original.swu.sig \
  -o modified.swu.sig
```

```bash
target/debug/sig repack ota-package-list.json \
  --template original-ota-package-list.json.sig \
  -o ota-package-list.json.sig
```

Template mode preserves metadata such as package type, version, IV, filename, and
unknown header fields, then rewrites the size, payload hash, and RSA signature.

To force encryption without a template:

```bash
target/debug/sig repack input-file --encrypt -o input-file.sig
```

Without `--encrypt` and without a template, `repack` writes a plain signed `.sig`.

## Unpack A Full OTA Update

The full OTA package has this structure:

```text
release.zip.sig
└── release.zip
    ├── cc2_...swu.sig
    │   └── cc2_...swu
    │       └── SWU CPIO archive contents
    └── ota-package-list.json.sig
        └── ota-package-list.json
```

Use the helper script:

```bash
scripts/undo-release-package.sh \
  cc2-01.03.02.36-ee76546c665bb272b43798813f60f8dd-release-abroad.zip.sig \
  release-unpacked
```

This creates:

```text
release-unpacked/
├── templates/
│   ├── release.zip.sig
│   ├── cc2_...swu.sig
│   └── ota-package-list.json.sig
├── release-signed/
│   ├── cc2_...swu.sig
│   └── ota-package-list.json.sig
├── release/
│   ├── cc2_...swu
│   └── ota-package-list.json
├── swu/
│   ├── .swu-manifest.json
│   ├── sw-description
│   ├── resource
│   ├── uboot
│   ├── boot0
│   ├── kernel
│   ├── rootfs
│   └── cpio_item_md5
└── repack.env
```

Edit files under `release-unpacked/swu/` to change SWU contents. For example, to
replace the rootfs image:

```bash
cp -p test/rootfs_modified release-unpacked/swu/rootfs
```

## Repack A Full OTA Update

After editing the unpacked contents, run:

```bash
scripts/redo-release-package.sh \
  release-unpacked \
  repacked.zip.sig
```

The script:

- Rebuilds the SWU CPIO archive from `release-unpacked/swu/`.
- Recomputes `cpio_item_md5`.
- Re-signs the SWU `.sig` using the original SWU `.sig` as a template.
- Updates the hash in `ota-package-list.json`.
- Re-signs `ota-package-list.json.sig` using its original template.
- Rebuilds the release ZIP while preserving original ZIP timestamps and modes.
- Re-signs the outer `.zip.sig` using the original outer `.sig` as a template.

If no files were changed, the rebuilt package should be bit-perfect:

```bash
cmp repacked.zip.sig original.zip.sig
```

If files were changed, the rebuilt package should differ, but all `.sig` headers
should still validate:

```bash
target/debug/sig info repacked.zip.sig
```

To validate inner signatures, unpack the release ZIP and run `info` on the inner
`.sig` files:

```bash
target/debug/sig unpack repacked.zip.sig -o /tmp/release.zip
unzip -q /tmp/release.zip -d /tmp/release
target/debug/sig info /tmp/release/cc2_eeb001_01.03.02.36_20260326171745.swu.sig
target/debug/sig info /tmp/release/ota-package-list.json.sig
```

Each should show:

```text
payload_sha256_matches: true
signature_valid: true
```

## Notes

- Inner `.sig` files are encrypted and signed.
- The outer `.zip.sig` is plain but signed.
- The RSA signature is stored in header bytes `0x100..0x200`.
- The signature signs the SHA-256 digest stored at `0xE0..0x100`.
- Reusing old signatures after modifying payloads will not work. Always use
  `repack` or `redo-release-package.sh` so signatures are regenerated.
