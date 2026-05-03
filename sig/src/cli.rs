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
/// The tool currently has one operation: `strip`. The enum layout leaves room
/// for future package inspection, signing, or repacking commands without
/// changing the top-level parser API.
#[derive(Subcommand)]
pub enum Command {
    /// Strip the .sig wrapper and write the unpacked payload.
    Strip(StripArgs),
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
