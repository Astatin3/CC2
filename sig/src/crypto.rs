//! Centauri Carbon 2 `.sig` package parsing, encryption, and decryption.
//!
//! The Carbon 2 update `.sig` container starts with a fixed 512-byte header.
//! The header begins with the big-endian magic value `ELEG`, carries a package
//! type byte whose high bit marks encrypted payloads, stores the unwrapped
//! payload size as a little-endian `u64`, and stores the AES-CBC IV at offset
//! `0xA0`. The wrapped payload starts immediately after the header.
//!
//! This module implements the same unpacking behavior as the public web tool at
//! <https://docs.opencentauri.cc/extras/cc2_update_decrypt.html>. It does not
//! attempt to validate signatures or hashes yet; it only removes the wrapper and
//! decrypts encrypted payloads.

use std::error::Error;

use aes::Aes256;
use cbc::cipher::{
    BlockDecryptMut, BlockEncryptMut, KeyIvInit,
    block_padding::{NoPadding, Pkcs7},
};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};

/// Length of the fixed `.sig` header in bytes.
pub const HEADER_LEN: usize = 512;

/// Big-endian `ELEG` magic value found at the beginning of supported packages.
const MAGIC: u32 = 0x454C4547;

/// AES-256-CBC key used by Carbon 2 update packages.
///
/// This key is taken from the referenced browser implementation. The IV is not
/// fixed; each package supplies its IV in the `.sig` header at bytes
/// `0xA0..0xB0`.
const AES_KEY: [u8; 32] = [
    0xD1, 0x4E, 0x15, 0x08, 0x43, 0xE9, 0xD1, 0x68, 0x93, 0x89, 0x07, 0x56, 0xD8, 0xF7, 0x7F, 0x67,
    0x4E, 0x16, 0x1A, 0x8B, 0xEB, 0xB8, 0xF7, 0x20, 0x73, 0x7E, 0xE6, 0x0E, 0x7F, 0x8C, 0x7E, 0x68,
];

type Aes256CbcDec = cbc::Decryptor<Aes256>;
type Aes256CbcEnc = cbc::Encryptor<Aes256>;

/// Parsed subset of a Carbon 2 `.sig` header.
///
/// The header contains additional metadata such as package type, version,
/// encrypted byte ranges, and SHA-256 material. `strip` only needs the fields
/// represented here: whether the payload is encrypted, how many bytes should be
/// emitted after unwrap, and which IV to use for AES-CBC.
#[derive(Debug)]
struct Header {
    /// Whether the high bit of the package type byte is set.
    is_encrypted: bool,

    /// Number of plaintext payload bytes to write after removing encryption and
    /// any trailing block padding.
    filesize: usize,

    /// Per-package AES-CBC initialization vector.
    iv: [u8; 16],
}

/// Unpack a complete `.sig` file held in memory.
///
/// The input must include both the 512-byte header and the wrapped payload. The
/// returned bytes are suitable to write directly as the stripped output package,
/// such as a `.swu`, `.zip`, or `.json` file.
///
/// For encrypted packages, this function decrypts the bytes after the header
/// with AES-256-CBC and then truncates the plaintext to the `filesize` recorded
/// in the header. For unencrypted packages, it simply copies exactly `filesize`
/// bytes from the post-header payload.
pub fn unpack_sig(raw: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    if raw.len() < HEADER_LEN {
        return Err("input is too small to contain a .sig header".into());
    }

    let header = parse_header(&raw[..HEADER_LEN])?;
    let payload = &raw[HEADER_LEN..];

    if header.is_encrypted {
        decrypt_payload(payload, &header)
    } else {
        strip_plain_payload(payload, header.filesize)
    }
}

/// Pack plaintext bytes into an encrypted `.sig` container.
///
/// The generated file is intentionally conservative: it writes the fields needed
/// by the known Carbon 2 unpacker and by [`unpack_sig`], but it does not claim to
/// recreate every vendor metadata field. The payload is encrypted with the fixed
/// Carbon 2 AES-256 key and a fresh random IV stored in the header.
pub fn pack_sig(payload: &[u8], filename: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut iv = [0u8; 16];
    OsRng.fill_bytes(&mut iv);

    pack_sig_with_iv(payload, filename, iv)
}

/// Pack plaintext bytes into an encrypted `.sig` container with a caller-supplied IV.
///
/// Normal callers should use [`pack_sig`], which generates a fresh random IV.
/// This function exists for reproducible output, especially regression tests that
/// need to prove the encryption path can recreate a known package byte-for-byte.
pub fn pack_sig_with_iv(
    payload: &[u8],
    filename: &str,
    iv: [u8; 16],
) -> Result<Vec<u8>, Box<dyn Error>> {
    pack_sig_inner(payload, filename, iv, None)
}

/// Pack plaintext bytes using the IV and opaque metadata from an existing `.sig` header.
///
/// The `.sig` header contains a 256-byte block at `0x100..0x200` that appears to
/// be signature or vendor metadata. The current tool cannot derive that block
/// from plaintext, so exact byte-for-byte reproduction of an existing `.sig`
/// requires copying it from a template. Known length, filename, IV, and encrypted
/// hash fields are still rewritten so the header matches the newly encrypted
/// payload.
pub fn pack_sig_with_template(
    payload: &[u8],
    filename: &str,
    template: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
    if template.len() < HEADER_LEN {
        return Err("template is too small to contain a .sig header".into());
    }

    let iv = template[0xA0..0xB0].try_into()?;
    pack_sig_inner(payload, filename, iv, Some(&template[..HEADER_LEN]))
}

fn pack_sig_inner(
    payload: &[u8],
    filename: &str,
    iv: [u8; 16],
    header_template: Option<&[u8]>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let filename_bytes = filename.as_bytes();
    if filename_bytes.len() >= 48 {
        return Err("header filename must be 47 bytes or shorter".into());
    }

    let ciphertext = encrypt_payload(payload, &iv)?;
    let encrypted_sha256 = Sha256::digest(&ciphertext).into();
    let mut output = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    output.extend_from_slice(&build_header(
        filename_bytes,
        payload.len(),
        ciphertext.len(),
        &iv,
        encrypted_sha256,
        header_template,
    )?);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

/// Build a 512-byte encrypted `.sig` header.
///
/// Field choices mirror the observed OTA package-list fixture where possible:
/// package type `0x83` marks an encrypted type-3 package, version bytes are
/// `1.2`, `0x08..0x10` stores the plaintext size, `0x98..0xA0` and
/// `0xB0..0xB8` store the encrypted payload length, and `0xA0..0xB0` stores the
/// IV. Bytes `0xE0..0x100` store the SHA-256 digest of the encrypted payload. If
/// a template is supplied, unknown metadata bytes are preserved from it.
fn build_header(
    filename: &[u8],
    payload_len: usize,
    encrypted_len: usize,
    iv: &[u8; 16],
    encrypted_sha256: [u8; 32],
    template: Option<&[u8]>,
) -> Result<[u8; HEADER_LEN], Box<dyn Error>> {
    let payload_len = u64::try_from(payload_len).map_err(|_| "payload is too large")?;
    let encrypted_len =
        u64::try_from(encrypted_len).map_err(|_| "encrypted payload is too large")?;

    let mut header = [0u8; HEADER_LEN];
    if let Some(template) = template {
        header.copy_from_slice(&template[..HEADER_LEN]);
    }

    header[0..4].copy_from_slice(&MAGIC.to_be_bytes());
    header[0x04] = 0x83;
    header[0x05] = 0x01;
    header[0x06] = 0x02;
    header[0x08..0x10].copy_from_slice(&payload_len.to_le_bytes());
    header[0x10..0x40].fill(0);
    header[0x10..0x10 + filename.len()].copy_from_slice(filename);
    header[0x98..0xA0].copy_from_slice(&encrypted_len.to_le_bytes());
    header[0xA0..0xB0].copy_from_slice(iv);
    header[0xB0..0xB8].copy_from_slice(&encrypted_len.to_le_bytes());
    header[0xE0..0x100].copy_from_slice(&encrypted_sha256);

    Ok(header)
}

/// Parse the fixed-width `.sig` header.
///
/// Offsets are based on the browser implementation:
///
/// - `0x00..0x04`: big-endian `ELEG` magic.
/// - `0x04`: package type; bit `0x80` indicates encryption.
/// - `0x08..0x10`: little-endian plaintext payload size.
/// - `0xA0..0xB0`: AES-CBC IV.
fn parse_header(header: &[u8]) -> Result<Header, Box<dyn Error>> {
    if header.len() < HEADER_LEN {
        return Err("header must be at least 512 bytes".into());
    }

    let magic = u32::from_be_bytes(header[0..4].try_into()?);
    if magic != MAGIC {
        return Err(format!("invalid .sig magic: 0x{magic:08x}").into());
    }

    let package_type = header[0x04];
    let is_encrypted = package_type & 0x80 != 0;
    let filesize = u64::from_le_bytes(header[0x08..0x10].try_into()?)
        .try_into()
        .map_err(|_| "payload filesize does not fit on this platform")?;
    let iv = header[0xA0..0xB0].try_into()?;

    Ok(Header {
        is_encrypted,
        filesize,
        iv,
    })
}

/// Return the plaintext payload for packages that are marked unencrypted.
///
/// Even unencrypted packages still include the `.sig` header. The header's
/// `filesize` field defines how many bytes after the header are part of the
/// actual package payload.
fn strip_plain_payload(payload: &[u8], filesize: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    if filesize > payload.len() {
        return Err(format!(
            "header payload size {} exceeds payload size {}",
            filesize,
            payload.len()
        )
        .into());
    }

    Ok(payload[..filesize].to_vec())
}

/// Decrypt and trim an encrypted `.sig` payload.
///
/// The browser version appends an extra encrypted PKCS#7 padding block to work
/// around Web Crypto's required padding behavior. Rust's `cbc` crate can decrypt
/// without padding, so this implementation decrypts the payload as-is and then
/// trims to the plaintext length declared in the header.
fn decrypt_payload(payload: &[u8], header: &Header) -> Result<Vec<u8>, Box<dyn Error>> {
    if payload.len() % 16 != 0 {
        return Err("encrypted payload length is not a multiple of the AES block size".into());
    }

    let mut buffer = payload.to_vec();
    let decrypted = Aes256CbcDec::new(&AES_KEY.into(), &header.iv.into())
        .decrypt_padded_mut::<NoPadding>(&mut buffer)
        .map_err(|_| "AES-CBC decryption failed")?;

    if header.filesize > decrypted.len() {
        return Err(format!(
            "header payload size {} exceeds decrypted payload size {}",
            header.filesize,
            decrypted.len()
        )
        .into());
    }

    Ok(decrypted[..header.filesize].to_vec())
}

/// Encrypt a plaintext payload with AES-256-CBC and no padding mode.
///
/// The observed Carbon 2 packages use PKCS#7 padding for encryption. The
/// original byte length is stored separately in the header, so [`unpack_sig`]
/// can decrypt without removing padding and then trim to the exact payload size.
fn encrypt_payload(payload: &[u8], iv: &[u8; 16]) -> Result<Vec<u8>, Box<dyn Error>> {
    let padded_len = payload.len().next_multiple_of(16) + 16;
    let mut buffer = vec![0u8; padded_len];
    buffer[..payload.len()].copy_from_slice(payload);

    let encrypted = Aes256CbcEnc::new(&AES_KEY.into(), iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, payload.len())
        .map_err(|_| "AES-CBC encryption failed")?;

    Ok(encrypted.to_vec())
}
