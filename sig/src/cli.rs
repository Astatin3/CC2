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
    /// Unpack the .sig wrapper and write the contained payload.
    #[command(alias = "strip")]
    Unpack(UnpackArgs),

    /// Print parsed .sig header information.
    Info(InfoArgs),

    /// Repack a file into a signed .sig container.
    #[command(alias = "encrypt")]
    Repack(RepackArgs),
}

/// Arguments accepted by the `info` subcommand.
#[derive(Args)]
pub struct InfoArgs {
    /// Input `.sig` file to inspect.
    pub input: PathBuf,

    /// Print exact hex bytes for unknown/reserved header regions.
    #[arg(long)]
    pub raw: bool,
}

/// Arguments accepted by the `unpack` subcommand.
///
/// The command mirrors the browser-based unpacker at
/// <https://docs.opencentauri.cc/extras/cc2_update_decrypt.html>: read a `.sig`
/// file, remove the header, decrypt the payload when needed, trim it to the
/// payload size declared in the header, then write the resulting package bytes.
#[derive(Args)]
pub struct UnpackArgs {
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

/// Arguments accepted by the `repack` subcommand.
///
/// This command performs the inverse of `unpack`: it writes a 512-byte `.sig`
/// header, signs the payload hash, and appends either plaintext or encrypted
/// payload bytes. When a template is supplied, unknown header metadata and the
/// template encryption mode are preserved by default.
#[derive(Args)]
pub struct RepackArgs {
    /// Input plaintext package file.
    ///
    /// With `--encrypt`, the bytes are copied into the encrypted payload with
    /// PKCS#7 padding. The original unpadded length is stored in the header so
    /// `unpack` can recover the exact input bytes.
    pub input: PathBuf,

    /// Output `.sig` path.
    ///
    /// When omitted, the command appends `.sig` to the input filename. For
    /// example, `update.swu` writes `update.swu.sig`.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Encrypt the payload before writing it.
    ///
    /// Without a template, repack writes a plain signed `.sig` unless this flag
    /// is set. With a template, the command preserves the template encryption
    /// mode unless this flag is set, in which case output is encrypted.
    #[arg(long)]
    pub encrypt: bool,

    /// Override the filename stored inside the `.sig` header.
    ///
    /// Defaults to the template header filename when `--template` is provided,
    /// otherwise to the input file name. The `.sig` format reserves 48 bytes for
    /// this field, so values longer than 47 bytes are rejected to preserve the
    /// trailing NUL terminator used by existing packages.
    #[arg(long)]
    pub filename: Option<String>,

    /// Existing `.sig` file whose header metadata should be reused.
    ///
    /// Use this when you need reproducible output that can match an existing
    /// encrypted package byte-for-byte. The command reuses the template IV and
    /// metadata, then rewrites size, filename, payload hash, and RSA signature.
    #[arg(long)]
    pub template: Option<PathBuf>,
}
