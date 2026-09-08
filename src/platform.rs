use std::path::Path;
use std::process::Command;

/// Open a file or directory with the operating system's default handler.
pub fn open_path(path: &Path) -> bool {
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

    command.spawn().is_ok()
}

/// Reveal a file or directory in the operating system's file manager.
///
/// macOS and Windows can select the exact item. On Linux there is no portable
/// "select this item" command, so this opens the containing directory instead.
pub fn reveal_path(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(path.as_os_str())
            .spawn()
            .is_ok()
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .is_ok()
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
