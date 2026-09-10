use std::path::{Path, PathBuf};
use std::process::Command;

/// Locate the bundled `assets` directory.
///
/// Bevy resolves its default asset root against the executable's directory,
/// which misses the repository copy when the binary is launched straight out of
/// `target/`. Probing the macOS bundle layout and then the executable's parent
/// directories keeps `cargo run`, a raw `target/<profile>/raptor`, and a bundled
/// `.app` all working from one build.
pub fn asset_root() -> PathBuf {
    let fallback = || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"));

    let Ok(executable) = std::env::current_exe() else {
        return fallback();
    };
    let Some(directory) = executable.parent() else {
        return fallback();
    };

    let bundled = directory.join("../Resources/assets");
    if bundled.is_dir() {
        return bundled;
    }

    directory
        .ancestors()
        .map(|ancestor| ancestor.join("assets"))
        .find(|candidate| candidate.is_dir())
        .unwrap_or_else(fallback)
}

/// Open a file or directory with the operating system's default handler.
pub fn open_path(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path.as_os_str());
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.arg("/C").arg("start").arg("").arg(path.as_os_str());
        command
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path.as_os_str());
        command
    };

    command.spawn().map(|_| ())
}

/// Reveal a file or directory in the operating system's file manager.
///
/// macOS and Windows can select the exact item. On Linux there is no portable
/// "select this item" command, so this opens the containing directory instead.
pub fn reveal_path(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(path.as_os_str())
            .spawn()
            .map(|_| ())
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map(|_| ())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        open_path(directory)
    }
}
