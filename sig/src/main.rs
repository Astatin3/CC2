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
        Command::Strip(args) => strip(args)?,
        Command::Encrypt(args) => encrypt(args)?,
    }

    Ok(())
}

/// Execute the `encrypt` subcommand.
///
/// The command reads a plaintext payload, wraps it with a fresh-IV encrypted
/// `.sig` header via [`crypto::pack_sig`], and writes the resulting container.
fn encrypt(args: cli::EncryptArgs) -> Result<(), Box<dyn Error>> {
    let raw = fs::read(&args.input)?;
    let filename = match args.filename {
        Some(filename) => filename,
        None => args
            .input
            .file_name()
            .ok_or("input path does not have a file name")?
            .to_string_lossy()
            .into_owned(),
    };
    let packed = if let Some(template) = args.template {
        let template = fs::read(template)?;
        crypto::pack_sig_with_template(&raw, &filename, &template)?
    } else {
        crypto::pack_sig(&raw, &filename)?
    };
    let output_path = args.output.unwrap_or_else(|| sig_output_path(&args.input));

    fs::write(&output_path, packed)?;
    println!("wrote {}", output_path.display());

    Ok(())
}

/// Execute the `strip` subcommand.
///
/// `strip` removes the 512-byte `.sig` wrapper and writes the contained package
/// bytes. For encrypted packages, [`crypto::unpack_sig`] performs the AES-CBC
/// decryption before returning the payload.
fn strip(args: cli::StripArgs) -> Result<(), Box<dyn Error>> {
    let raw = fs::read(&args.input)?;
    let unpacked = crypto::unpack_sig(&raw)?;
    let output_path = args
        .output
        .unwrap_or_else(|| default_output_path(&args.input));

    fs::write(&output_path, unpacked)?;
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
