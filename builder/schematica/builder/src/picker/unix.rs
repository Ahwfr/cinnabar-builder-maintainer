//! Desktop file picker and notices through the zenity or kdialog helpers.

use anyhow::{Context, Result, bail};
use std::{
    ffi::OsStr,
    io::ErrorKind,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    process::{Command, Output},
};

const TITLE: &str = "Select up to 8 schematics for one library";
const PATTERNS: &str = "*.schematic *.schem *.litematic *.mcstructure";

pub fn run() {
    match choose().and_then(|inputs| {
        inputs
            .map(|inputs| super::convert_selection(&inputs))
            .transpose()
    }) {
        Ok(Some(output)) => message(
            &format!(
                "Created {}\n\nImport this file in Cinnabar > Settings > Mods.",
                output.display()
            ),
            false,
        ),
        Ok(None) => {}
        Err(error) => show_error(&format!("{error:#}")),
    }
}

pub fn show_error(error: &str) {
    message(error, true);
}

/// Returns selected existing source paths without converting or changing them.
///
/// `Ok(None)` means the player cancelled. Fails when neither helper is installed.
pub fn choose() -> Result<Option<Vec<PathBuf>>> {
    let zenity = Command::new("zenity")
        .args(["--file-selection", "--multiple", "--separator=\n"])
        .arg(format!("--title={TITLE}"))
        .arg(format!("--file-filter=Minecraft schematics | {PATTERNS}"))
        .output();
    let output = match helper(zenity)? {
        Some(output) => output,
        None => {
            let kdialog = Command::new("kdialog")
                .args([
                    "--getopenfilename",
                    ".",
                    PATTERNS,
                    "--multiple",
                    "--separate-output",
                ])
                .args(["--title", TITLE])
                .output();
            helper(kdialog)?.context(
                "Install zenity or kdialog to choose schematics, or pass the files to `schematica-builder library`.",
            )?
        }
    };
    // Both helpers exit with status 1 when the dialog is cancelled.
    if !output.status.success() {
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        bail!(
            "The file picker failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let inputs: Vec<_> = output
        .stdout
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| PathBuf::from(OsStr::from_bytes(line)))
        .collect();
    Ok((!inputs.is_empty()).then_some(inputs))
}

/// Maps a missing helper to `None` so the caller can try the next one.
fn helper(result: std::io::Result<Output>) -> Result<Option<Output>> {
    match result {
        Ok(output) => Ok(Some(output)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("Start the file picker"),
    }
}

/// Shows a modal notice, falling back to stderr when no helper is installed.
fn message(body: &str, error: bool) {
    let zenity = Command::new("zenity")
        .arg(if error { "--error" } else { "--info" })
        .args(["--title=Schematica Import", "--no-markup"])
        .arg(format!("--text={body}"))
        .status();
    if zenity.is_ok() {
        return;
    }
    let kdialog = Command::new("kdialog")
        .arg(if error { "--error" } else { "--msgbox" })
        .arg(body)
        .args(["--title", "Schematica Import"])
        .status();
    if kdialog.is_err() {
        eprintln!("{body}");
    }
}
