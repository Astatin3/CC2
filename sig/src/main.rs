//! Command-line entry point for the `sig` utility.
//!
//! The binary intentionally keeps very little logic here. Clap-owned command
//! definitions live in [`cli`], while the Centauri Carbon 2 `.sig` format and
//! cryptographic details live in [`crypto`]. That split keeps argument parsing
//! separate from file-format handling so future commands can reuse the same
//! unpacking code without duplicating CLI concerns.

use std::{error::Error, fs, path::PathBuf};

use clap::Parser;

mod cli;
mod crypto;

#[cfg(test)]
mod tests;

use cli::{Cli, Command};

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Unpack(args) => unpack(args)?,
        Command::Info(args) => info(args)?,
        Command::Repack(args) => repack(args)?,
    }

    Ok(())
}

/// Execute the `info` subcommand.
fn info(args: cli::InfoArgs) -> Result<(), Box<dyn Error>> {
    let raw = fs::read(&args.input)?;
    let info = crypto::header_info(&raw)?;

    println!("file: {}", args.input.display());
    println!("total_len: {}", info.total_len);
    println!("header_len: {}", crypto::HEADER_LEN);
    println!("payload_len: {}", info.payload_len);
    println!(
        "magic: {}",
        std::str::from_utf8(&info.magic).unwrap_or("<non-ascii>")
    );
    println!("package_type_raw: 0x{:02x}", info.package_type_raw);
    println!("package_type: {}", info.package_type);
    println!("encrypted: {}", info.is_encrypted);
    println!("version: {}.{}", info.version_major, info.version_minor);
    println!("byte_07: 0x{:02x}", info.byte_07);
    println!("filesize: {}", info.filesize);
    println!("filename: {}", info.filename);
    print_region("reserved_40_90", &info.reserved_40_90, args.raw);
    println!("encrypt_offset: {}", info.encrypt_offset);
    println!("encrypt_length: {}", info.encrypt_length);
    println!("iv: {}", info.iv_hex);
    println!("encrypt_filesize: {}", info.encrypt_filesize);
    print_region("reserved_b8_e0", &info.reserved_b8_e0, args.raw);
    println!("stored_sha256: {}", info.stored_sha256_hex);
    println!("payload_sha256: {}", info.payload_sha256_hex);
    println!("payload_sha256_matches: {}", info.payload_sha256_matches);
    println!("signature_valid: {}", info.signature_valid);
    print_region("opaque_100_200", &info.opaque_100_200, args.raw);

    Ok(())
}

fn print_region(name: &str, region: &crypto::RegionInfo, raw: bool) {
    println!("{name}_len: {}", region.len);
    println!("{name}_nonzero_count: {}", region.nonzero_count);
    println!("{name}_sha256: {}", region.sha256_hex);
    if raw {
        println!("{name}_bytes_hex:");
        for chunk in region.bytes_hex.as_bytes().chunks(64) {
            println!("  {}", std::str::from_utf8(chunk).unwrap_or(""));
        }
    }
}

/// Execute the `repack` subcommand.
///
/// The command reads a plaintext payload, wraps it in a signed `.sig` header,
/// and optionally encrypts the payload.
fn repack(args: cli::RepackArgs) -> Result<(), Box<dyn Error>> {
    let raw = fs::read(&args.input)?;
    let template = match args.template {
        Some(template) => Some(fs::read(template)?),
        None => None,
    };
    let filename = match (args.filename, template.as_deref()) {
        (Some(filename), _) => filename,
        (None, Some(template)) => crypto::header_filename(template)?,
        (None, None) => args
            .input
            .file_name()
            .ok_or("input path does not have a file name")?
            .to_string_lossy()
            .into_owned(),
    };
    let packed = if let Some(template) = template {
        let encrypt = if args.encrypt { Some(true) } else { None };
        crypto::pack_sig_with_template(&raw, &filename, &template, encrypt)?
    } else if args.encrypt {
        crypto::pack_sig_encrypted(&raw, &filename)?
    } else {
        crypto::pack_sig(&raw, &filename)?
    };
    let output_path = args.output.unwrap_or_else(|| sig_output_path(&args.input));

    fs::write(&output_path, packed)?;
    println!("wrote {}", output_path.display());

    Ok(())
}

/// Execute the `unpack` subcommand.
///
/// `unpack` removes the 512-byte `.sig` wrapper and writes the contained package
/// bytes. For encrypted packages, [`crypto::unpack_sig`] performs the AES-CBC
/// decryption before returning the payload.
fn unpack(args: cli::UnpackArgs) -> Result<(), Box<dyn Error>> {
    let raw = fs::read(&args.input)?;
    let filename = crypto::header_filename(&raw)?;
    let unpacked = crypto::unpack_sig(&raw)?;
    let output_path = args
        .output
        .unwrap_or_else(|| default_output_path(&args.input));

    fs::write(&output_path, unpacked)?;
    println!("header filename: {filename}");
    println!("wrote {}", output_path.display());

    Ok(())
}

/// Derive the default output path used by the web unpacker.
///
/// A conventional input like `firmware.zip.sig` becomes `firmware.zip`. If the
/// input does not use the `.sig` extension, the tool still writes beside the
/// original file but changes the extension to `.decrypted` to avoid overwriting
/// the input by accident.
fn default_output_path(input: &std::path::Path) -> PathBuf {
    if input.extension().is_some_and(|ext| ext == "sig") {
        input.with_extension("")
    } else {
        input.with_extension("decrypted")
    }
}

/// Derive the default output path for `encrypt`.
///
/// Unlike [`default_output_path`], encryption appends an extension instead of
/// replacing one: `firmware.zip` becomes `firmware.zip.sig`.
fn sig_output_path(input: &std::path::Path) -> PathBuf {
    let mut filename = input
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "output".into());
    filename.push(".sig");
    input.with_file_name(filename)
}
