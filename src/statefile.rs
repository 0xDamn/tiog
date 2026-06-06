//! Helpers for writing local state files without making them world-readable.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub fn create_private(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);

    #[cfg(unix)]
    {
        options.mode(0o600);
    }

    let file = options.open(path)?;

    #[cfg(unix)]
    {
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }

    Ok(file)
}

pub fn create_private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;

    #[cfg(unix)]
    {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }

    Ok(())
}

pub fn state_dir() -> Option<PathBuf> {
    Some(match std::env::var("XDG_STATE_HOME") {
        Ok(x) if !x.is_empty() => PathBuf::from(x),
        _ => dirs::home_dir()?.join(".local/state"),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn create_private_uses_user_only_permissions() {
        let path = std::env::temp_dir().join(format!("tiog-statefile-test-{}", std::process::id()));

        {
            let mut file = create_private(&path).unwrap();
            writeln!(file, "state").unwrap();
        }

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let _ = std::fs::remove_file(&path);

        assert_eq!(mode, 0o600);
    }

    #[test]
    fn create_private_dir_uses_user_only_permissions() {
        let path =
            std::env::temp_dir().join(format!("tiog-statefile-dir-test-{}", std::process::id()));

        create_private_dir(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let _ = std::fs::remove_dir(&path);

        assert_eq!(mode, 0o700);
    }
}
