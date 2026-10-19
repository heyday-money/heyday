use std::{fs::{File, OpenOptions}, io, path::Path};

// Keep this handle alive until shutdown. Never unlink the lock file: doing so
// would allow another process to lock a different file at the same path.
pub struct DatabaseInstance {
    _lock: File,
}

impl DatabaseInstance {
    pub fn acquire(directory: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).create(true)
            .truncate(false).open(directory.join("instance.lock"))?;
        file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => io::Error::new(
                io::ErrorKind::WouldBlock,
                "Heyday Money is already using this database. Close the other instance before opening it again.",
            ),
            std::fs::TryLockError::Error(error) => error,
        })?;
        Ok(Self { _lock: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_cannot_open_locked_database() {
        let Some(directory) = std::env::var_os("HEYDAY_INSTANCE_TEST_DIRECTORY") else { return };
        assert_eq!(DatabaseInstance::acquire(Path::new(&directory)).err().unwrap().kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn excludes_other_processes_and_releases_on_close() {
        let directory = tempfile::tempdir().unwrap();
        let instance = DatabaseInstance::acquire(directory.path()).unwrap();
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "instance::tests::child_cannot_open_locked_database"])
            .env("HEYDAY_INSTANCE_TEST_DIRECTORY", directory.path()).status().unwrap();
        assert!(child.success());
        // Debug/release databases can be open independently.
        let other = tempfile::tempdir().unwrap();
        let _other_instance = DatabaseInstance::acquire(other.path()).unwrap();
        drop(instance);
        let _reopened = DatabaseInstance::acquire(directory.path()).unwrap();
    }
}
