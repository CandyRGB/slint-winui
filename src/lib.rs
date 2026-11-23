//! Paths and build helpers for embedding the Slint components in a Rust host.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(all(windows, feature = "windows-platform"))]
pub mod platform;

/// Map a Slint library alias, such as `winui`, to this file.
pub fn library_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/lib.slint")
}

/// Write registrations selected by the host into its build output directory.
///
/// Include the resulting file at the Rust crate root, alongside
/// `slint::include_modules!()`. Re-export each behavior's Slint types from the
/// host's entry point so it and the adapter share the same generated Rust types.
pub fn write_behavior_modules(out_dir: impl AsRef<Path>, modules: &[&str]) -> io::Result<PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/behavior");
    let mut declarations = String::new();
    for module in modules.iter().copied().collect::<BTreeSet<_>>() {
        if module.is_empty()
            || !module
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid behavior module name: {module:?}"),
            ));
        }
        let file = directory.join(format!("{module}.rs"));
        if !file.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Unknown Slint WinUI behavior: {module}"),
            ));
        }
        writeln!(declarations, "#[path = {file:?}]\npub mod {module};")
            .expect("writing to a String cannot fail");
        println!("cargo:rerun-if-changed={}", file.display());
    }
    let output = out_dir.as_ref().join("slint_winui_behaviors.rs");
    fs::write(&output, declarations)?;
    Ok(output)
}
