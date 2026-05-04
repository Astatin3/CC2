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
use rsa::{
    RsaPrivateKey, RsaPublicKey,
    pkcs1v15::Pkcs1v15Sign,
    pkcs8::{DecodePrivateKey, DecodePublicKey},
};
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

/// RSA private key used to sign the SHA-256 digest stored in `.sig` headers.
const RSA_PRIVATE_KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCgET+oNgUA1acA
vyESnNsTy+Ae+k6Q/LLM5bh4sLfFeu0TT9Ka4SQa6LffDLJvaqjt++jsB2PwPvZc
clHcG5yC2bNikz+x2WaHCQOI26vzy8oh8x+wLI77GfVG82mWpo6gHfyu51LBvCDo
aDfghSUL7B9lIaCDb0dbBfpjTTrQr0UcUET9RvsrjMkyLI5jfBnd1VNTvFD9gngc
wAmZ7s65UIEkKjmQZH+xK+gDa+oZBJpjz7vgXEezIVSDuZQKQFB/4E5YErftRwNp
h7rngDTpyFYjaAvD9ZoM90exFWtRWMvjt2HNtDUOHjRI4C5XToqm/FfPxYAproJU
PyE6ckFrAgMBAAECggEAIzAHhWvyp59QKiraE2RmCLEN4OF3ugnDKKXraqS2kXQX
f+JRUvjhXgUAvsjkxPd2kXKKXrC1OJAuyl3bPv7W5jEDbU0feHJpRpAltcVMxLht
BA+VTL5O5EZtlB5YfOS6f9p3vN9fYvV/anfWqMW8QiWzNSEyTxJ8ZjcnNwM4Rb2Y
hMxDpQ8EMIK4Hwchmv8V2vNIY1CnIFz+i1U722PnnR6wDO4obFX2wh+6KCBuTrwW
babpNhGpW2P/f33HoeVhoJkt4k7kogM45cqPwC9i3YKpEgeHdo6aRs+h/hhFk9BV
NKks3lK6kY20KvGLyhauh9usKZRiONxn/w0YmB4CeQKBgQC2tuIqXg1KARaZ7FkY
Bw/2sr2uCbiN3vgP30RMWNtrtCluZ49HSphV8Td69qgI6hXyFBzm1uaLA8gAZ6DR
r9VYu6uQTLZDhMkd36pbcCQsvmF1Q9yifBd3nBHZWt6eek61mbK0MrBq2ofEDSAd
dPV1VEAMthB20yAT7qI33XLj7QKBgQDgRPhATfkdVcRE0D4+XkIUmNUF7MLzRfxq
ZvXhhxci0d2TtkIKhMzNFQrmIaIp8ex8mpbmyebmsLX6lINjvQAzmHGuk5niOR8w
T5Lkj8WY9E7lw+clApo5TXLJunHxEdtamASdjRdI1lAGNhG4SzH/ea6OtR/HeziA
kQ6NMOI/twKBgBlQjVVBYqX2MKNy04U4tUWAzjbmseM2GThZvqS1SvFJLNRXFMrT
0vdVTFKFChLyG8hGcRqqe5aXF4a21Nk4e16n4cVEW5xPMW4qJvg0OU7ZsbcFh/Qb
LUUtImvy4xUh7PXMLa45t6eWT2kiSGjMY5W17onUT8OmzLL2RRNoYxqhAoGABLZO
RQOeZVhk/FEnzaWrW8VuTGaSHgxtZkrthaSR/uBL+IuOzavGpdR4Wyd/wcPchS22
V/kMCfLSkAZI0HKrK2pbkSB2zkMG/bveSUEgFLulYLyCAcwRM30GGWj6dec7Jacm
Ca1qPNSL7+V479dcoJKM8WCq30Uehc0Gcj8BsfcCgYBTicTc3S/o2jsJLRtXzFap
eooocDD8Q0xIFRMZplDOvAnWXq6QJ0Xyeqlx9dk41dwWvnHHJGazZPZA4wijyI0T
orTbpLkOKpMz0hlqy9j2/k+vgf004+xfDSAywtOPop9csXxFWPB5qmdjTONv5Zpn
bzzXqOTAU1QZvdTdOBvbUw==
-----END PRIVATE KEY-----"#;

/// RSA public key used to verify the header signature.
const RSA_PUBLIC_KEY_PEM: &str = r#"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAoBE/qDYFANWnAL8hEpzb
E8vgHvpOkPyyzOW4eLC3xXrtE0/SmuEkGui33wyyb2qo7fvo7Adj8D72XHJR3Buc
gtmzYpM/sdlmhwkDiNur88vKIfMfsCyO+xn1RvNplqaOoB38rudSwbwg6Gg34IUl
C+wfZSGgg29HWwX6Y0060K9FHFBE/Ub7K4zJMiyOY3wZ3dVTU7xQ/YJ4HMAJme7O
uVCBJCo5kGR/sSvoA2vqGQSaY8+74FxHsyFUg7mUCkBQf+BOWBK37UcDaYe654A0
6chWI2gLw/WaDPdHsRVrUVjL47dhzbQ1Dh40SOAuV06KpvxXz8WAKa6CVD8hOnJB
awIDAQAB
-----END PUBLIC KEY-----"#;

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

/// Human-readable view of every known `.sig` header field.
///
/// The format still has unknown regions. Those are exposed as non-zero counts
/// and SHA-256 digests so different files can be compared without dumping large
/// binary blobs to the terminal.
#[derive(Debug)]
pub struct HeaderInfo {
    /// Total input file length.
    pub total_len: usize,
    /// Number of bytes after the fixed 512-byte header.
    pub payload_len: usize,
    /// Big-endian magic as raw bytes.
    pub magic: [u8; 4],
    /// Raw package type byte at `0x04`.
    pub package_type_raw: u8,
    /// Low seven bits of the package type byte.
    pub package_type: u8,
    /// Whether the high bit of the package type byte is set.
    pub is_encrypted: bool,
    /// Header version major byte at `0x05`.
    pub version_major: u8,
    /// Header version minor byte at `0x06`.
    pub version_minor: u8,
    /// Currently unknown byte at `0x07`.
    pub byte_07: u8,
    /// Plaintext payload size from `0x08..0x10`.
    pub filesize: u64,
    /// NUL-terminated header filename from `0x10..0x40`.
    pub filename: String,
    /// Unknown/reserved bytes from `0x40..0x90`.
    pub reserved_40_90: RegionInfo,
    /// Encryption offset from `0x90..0x98`.
    pub encrypt_offset: u64,
    /// Encrypted payload length from `0x98..0xA0`.
    pub encrypt_length: u64,
    /// AES-CBC IV from `0xA0..0xB0`.
    pub iv_hex: String,
    /// Encrypted payload length/size mirror from `0xB0..0xB8`.
    pub encrypt_filesize: u64,
    /// Unknown/reserved bytes from `0xB8..0xE0`.
    pub reserved_b8_e0: RegionInfo,
    /// Stored SHA-256 digest from `0xE0..0x100`.
    pub stored_sha256_hex: String,
    /// SHA-256 of the actual bytes after the header.
    pub payload_sha256_hex: String,
    /// Whether `stored_sha256_hex` matches `payload_sha256_hex`.
    pub payload_sha256_matches: bool,
    /// Whether the RSA signature at `0x100..0x200` verifies the stored SHA-256.
    pub signature_valid: bool,
    /// Opaque 256-byte region from `0x100..0x200`.
    pub opaque_100_200: RegionInfo,
}

/// Summary for an unknown or reserved header byte range.
#[derive(Debug)]
pub struct RegionInfo {
    /// Number of bytes in the region.
    pub len: usize,
    /// Number of non-zero bytes in the region.
    pub nonzero_count: usize,
    /// SHA-256 digest of the region.
    pub sha256_hex: String,
    /// Exact region bytes as lowercase hexadecimal.
    pub bytes_hex: String,
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

/// Return the NUL-terminated filename stored in a `.sig` header.
pub fn header_filename(raw: &[u8]) -> Result<String, Box<dyn Error>> {
    if raw.len() < HEADER_LEN {
        return Err("input is too small to contain a .sig header".into());
    }

    let header = &raw[..HEADER_LEN];
    let magic = u32::from_be_bytes(header[0..4].try_into()?);
    if magic != MAGIC {
        return Err(format!("invalid .sig magic: 0x{magic:08x}").into());
    }

    let filename = &header[0x10..0x40];
    let end = filename
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(filename.len());

    Ok(std::str::from_utf8(&filename[..end])?.to_owned())
}

/// Parse and summarize all known `.sig` header fields.
pub fn header_info(raw: &[u8]) -> Result<HeaderInfo, Box<dyn Error>> {
    if raw.len() < HEADER_LEN {
        return Err("input is too small to contain a .sig header".into());
    }

    let header = &raw[..HEADER_LEN];
    let magic = header[0..4].try_into()?;
    if u32::from_be_bytes(magic) != MAGIC {
        return Err(format!("invalid .sig magic: 0x{:08x}", u32::from_be_bytes(magic)).into());
    }

    let package_type_raw = header[0x04];
    let payload = &raw[HEADER_LEN..];
    let stored_sha256_hex = bytes_to_hex(&header[0xE0..0x100]);
    let payload_sha256_hex = bytes_to_hex(&Sha256::digest(payload));
    let payload_sha256_matches = stored_sha256_hex == payload_sha256_hex;

    Ok(HeaderInfo {
        total_len: raw.len(),
        payload_len: payload.len(),
        magic,
        package_type_raw,
        package_type: package_type_raw & 0x7F,
        is_encrypted: package_type_raw & 0x80 != 0,
        version_major: header[0x05],
        version_minor: header[0x06],
        byte_07: header[0x07],
        filesize: u64::from_le_bytes(header[0x08..0x10].try_into()?),
        filename: header_filename(raw)?,
        reserved_40_90: region_info(&header[0x40..0x90]),
        encrypt_offset: u64::from_le_bytes(header[0x90..0x98].try_into()?),
        encrypt_length: u64::from_le_bytes(header[0x98..0xA0].try_into()?),
        iv_hex: bytes_to_hex(&header[0xA0..0xB0]),
        encrypt_filesize: u64::from_le_bytes(header[0xB0..0xB8].try_into()?),
        reserved_b8_e0: region_info(&header[0xB8..0xE0]),
        stored_sha256_hex,
        payload_sha256_hex,
        payload_sha256_matches,
        signature_valid: verify_payload_signature(&header[0xE0..0x100], &header[0x100..0x200]),
        opaque_100_200: region_info(&header[0x100..0x200]),
    })
}

fn region_info(bytes: &[u8]) -> RegionInfo {
    RegionInfo {
        len: bytes.len(),
        nonzero_count: bytes.iter().filter(|byte| **byte != 0).count(),
        sha256_hex: bytes_to_hex(&Sha256::digest(bytes)),
        bytes_hex: bytes_to_hex(bytes),
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}

/// Pack plaintext bytes into a plain signed `.sig` container.
///
/// The generated file is intentionally conservative: it writes the fields needed
/// by the known Carbon 2 unpacker and by [`unpack_sig`], but it does not claim to
/// recreate every vendor metadata field.
pub fn pack_sig(payload: &[u8], filename: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    pack_sig_inner(payload, filename, None, None, false)
}

/// Pack plaintext bytes into an encrypted signed `.sig` container.
pub fn pack_sig_encrypted(payload: &[u8], filename: &str) -> Result<Vec<u8>, Box<dyn Error>> {
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
    pack_sig_inner(payload, filename, Some(iv), None, true)
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
    encrypt: Option<bool>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    if template.len() < HEADER_LEN {
        return Err("template is too small to contain a .sig header".into());
    }

    let template_header = &template[..HEADER_LEN];
    let template_is_encrypted = template_header[0x04] & 0x80 != 0;
    let encrypt = encrypt.unwrap_or(template_is_encrypted);
    let iv = if encrypt {
        Some(template_header[0xA0..0xB0].try_into()?)
    } else {
        None
    };
    pack_sig_inner(payload, filename, iv, Some(template_header), encrypt)
}

fn pack_sig_inner(
    payload: &[u8],
    filename: &str,
    iv: Option<[u8; 16]>,
    header_template: Option<&[u8]>,
    encrypt: bool,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let filename_bytes = filename.as_bytes();
    if filename_bytes.len() >= 48 {
        return Err("header filename must be 47 bytes or shorter".into());
    }

    let payload_out = if encrypt {
        let iv = iv.ok_or("encrypted repack requires an IV")?;
        encrypt_payload(payload, &iv)?
    } else {
        payload.to_vec()
    };
    let payload_sha256 = Sha256::digest(&payload_out).into();
    let mut output = Vec::with_capacity(HEADER_LEN + payload_out.len());
    output.extend_from_slice(&build_header(
        filename_bytes,
        payload.len(),
        payload_out.len(),
        iv,
        payload_sha256,
        header_template,
        encrypt,
    )?);
    output.extend_from_slice(&payload_out);

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
    output_payload_len: usize,
    iv: Option<[u8; 16]>,
    payload_sha256: [u8; 32],
    template: Option<&[u8]>,
    encrypt: bool,
) -> Result<[u8; HEADER_LEN], Box<dyn Error>> {
    let payload_len = u64::try_from(payload_len).map_err(|_| "payload is too large")?;
    let output_payload_len =
        u64::try_from(output_payload_len).map_err(|_| "output payload is too large")?;

    let mut header = [0u8; HEADER_LEN];
    if let Some(template) = template {
        header.copy_from_slice(&template[..HEADER_LEN]);
    }

    header[0..4].copy_from_slice(&MAGIC.to_be_bytes());
    if template.is_none() || (header[0x04] & 0x80 != 0) != encrypt {
        header[0x04] = if encrypt { 0x83 } else { 0x04 };
        header[0x05] = 0x01;
        header[0x06] = 0x02;
    }
    header[0x08..0x10].copy_from_slice(&payload_len.to_le_bytes());
    header[0x10..0x40].fill(0);
    header[0x10..0x10 + filename.len()].copy_from_slice(filename);
    if encrypt {
        let iv = iv.ok_or("encrypted header requires an IV")?;
        header[0x98..0xA0].copy_from_slice(&output_payload_len.to_le_bytes());
        header[0xA0..0xB0].copy_from_slice(&iv);
        header[0xB0..0xB8].copy_from_slice(&output_payload_len.to_le_bytes());
    } else {
        header[0x98..0xA0].fill(0);
        header[0xA0..0xB0].fill(0);
        header[0xB0..0xB8].copy_from_slice(&payload_len.to_le_bytes());
    }
    header[0xE0..0x100].copy_from_slice(&payload_sha256);
    let signature = sign_payload_hash(&payload_sha256)?;
    header[0x100..0x200].copy_from_slice(&signature);

    Ok(header)
}

fn sign_payload_hash(payload_sha256: &[u8; 32]) -> Result<Vec<u8>, Box<dyn Error>> {
    let key = RsaPrivateKey::from_pkcs8_pem(RSA_PRIVATE_KEY_PEM)?;
    Ok(key.sign(Pkcs1v15Sign::new::<Sha256>(), payload_sha256)?)
}

fn verify_payload_signature(payload_sha256: &[u8], signature: &[u8]) -> bool {
    let Ok(key) = RsaPublicKey::from_public_key_pem(RSA_PUBLIC_KEY_PEM) else {
        return false;
    };
    key.verify(Pkcs1v15Sign::new::<Sha256>(), payload_sha256, signature)
        .is_ok()
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
