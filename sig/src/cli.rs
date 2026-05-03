//! Clap command and argument definitions.
//!
//! This module is deliberately limited to command-line shape: subcommand names,
//! flags, positional arguments, and user-facing help text. It does not know how
//! to parse or decrypt `.sig` files. Keeping those responsibilities out of the
//! CLI layer makes the format code easier to test and reuse from future commands.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Top-level command-line parser for the `sig` binary.
///
/// `clap` derives `--help`, `--version`, shell-friendly error messages, and the
/// dispatch table for all subcommands from this struct. New subcommands should
/// be added to [`Command`] rather than handled manually in `main`.
#[derive(Parser)]
#[command(version, about = "Interact with Centauri Carbon 2 .sig packages")]
pub struct Cli {
    /// The action to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Supported subcommands.
///
/// The tool currently supports unpacking and re-encrypting package payloads.
/// The enum layout leaves room for future package inspection, signing, or
/// repacking commands without changing the top-level parser API.
#[derive(Subcommand)]
pub enum Command {
    /// Strip the .sig wrapper and write the unpacked payload.
    Strip(StripArgs),

    /// Encrypt a file and wrap it in a .sig container.
    Encrypt(EncryptArgs),
}

/// Arguments accepted by the `strip` subcommand.
///
/// The command mirrors the browser-based unpacker at
/// <https://docs.opencentauri.cc/extras/cc2_update_decrypt.html>: read a `.sig`
/// file, remove the header, decrypt the payload when needed, trim it to the
/// payload size declared in the header, then write the resulting package bytes.
#[derive(Args)]
pub struct StripArgs {
    /// Input `.sig` file.
    ///
    /// The file must begin with the Centauri/Elegoo `ELEG` magic value and must
    /// contain the 512-byte header used by Carbon 2 update packages.
    pub input: PathBuf,

    /// Output path.
    ///
    /// When omitted, the default output is the input filename with a trailing
    /// `.sig` extension removed. For example, `update.swu.sig` writes
    /// `update.swu`.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Arguments accepted by the `encrypt` subcommand.
///
/// This command performs the inverse of `strip` for the subset of the format the
/// tool understands: it encrypts a plaintext payload with the Carbon 2 AES key,
/// writes a 512-byte `.sig` header, and appends the ciphertext. It does not yet
/// create or verify any vendor signature material beyond the metadata needed for
/// decryption.
#[derive(Args)]
pub struct EncryptArgs {
    /// Input plaintext package file.
    ///
    /// The bytes are copied into the encrypted payload with PKCS#7 padding. The
    /// original unpadded length is stored in the header so `strip` can recover
    /// the exact input bytes.
    pub input: PathBuf,

    /// Output `.sig` path.
    ///
    /// When omitted, the command appends `.sig` to the input filename. For
    /// example, `update.swu` writes `update.swu.sig`.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Filename to store inside the `.sig` header.
    ///
    /// Defaults to the input file name. The `.sig` format reserves 48 bytes for
    /// this field, so values longer than 47 bytes are rejected to preserve the
    /// trailing NUL terminator used by existing packages.
    #[arg(long)]
    pub filename: Option<String>,

    /// Existing `.sig` file whose header metadata should be reused.
    ///
    /// Use this when you need reproducible output that can match an existing
    /// encrypted package byte-for-byte. The command reuses the template IV and
    /// opaque header metadata, then rewrites size, filename, and payload hash
    /// fields for the new encrypted payload.
    #[arg(long)]
    pub template: Option<PathBuf>,
}
