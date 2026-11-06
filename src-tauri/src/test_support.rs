//! Strict cleanup for temporary files owned by native tests.
//!
//! Callers must close their database connections first. Windows can still report
//! a brief sharing/lock violation during cleanup (for example, an external file
//! scanner may hold a handle). Retry only those errors, with a finite bound.
//! Permanent failures must fail the test rather than silently leak its files.

use std::{io, path::Path, time::Duration};

const CLEANUP_ATTEMPTS: usize = 40;
const CLEANUP_RETRY_DELAY: Duration = Duration::from_millis(50);

pub async fn remove_file(path: &Path) -> io::Result<()> {
    retry_cleanup(|| std::fs::remove_file(path), cfg!(windows)).await
}

pub async fn remove_dir_all(path: &Path) -> io::Result<()> {
    retry_cleanup(|| std::fs::remove_dir_all(path), cfg!(windows)).await
}

async fn retry_cleanup(
    mut remove: impl FnMut() -> io::Result<()>,
    windows: bool,
) -> io::Result<()> {
    for attempt in 1..=CLEANUP_ATTEMPTS {
        match remove() {
            Ok(()) => return Ok(()),
            Err(error)
                if windows
                    && matches!(error.raw_os_error(), Some(32) | Some(33))
                    && attempt < CLEANUP_ATTEMPTS =>
            {
                tokio::time::sleep(CLEANUP_RETRY_DELAY).await;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("the last cleanup attempt always returns")
}

#[tokio::test]
async fn cleanup_retries_only_windows_sharing_and_lock_violations() {
    for code in [32, 33] {
        let mut attempts = 0;
        retry_cleanup(
            || {
                attempts += 1;
                if attempts < 3 {
                    Err(io::Error::from_raw_os_error(code))
                } else {
                    Ok(())
                }
            },
            true,
        )
        .await
        .unwrap();
        assert_eq!(attempts, 3);
    }
    for (windows, code) in [(true, 5), (true, 2), (false, 32), (false, 33)] {
        let mut attempts = 0;
        let error = retry_cleanup(
            || {
                attempts += 1;
                Err(io::Error::from_raw_os_error(code))
            },
            windows,
        )
        .await
        .unwrap_err();
        assert_eq!(attempts, 1);
        assert_eq!(error.raw_os_error(), Some(code));
    }
}

#[tokio::test]
async fn cleanup_fails_when_a_lock_persists() {
    let mut attempts = 0;
    let error = retry_cleanup(
        || {
            attempts += 1;
            Err(io::Error::from_raw_os_error(32))
        },
        true,
    )
    .await
    .unwrap_err();
    assert_eq!(attempts, CLEANUP_ATTEMPTS);
    assert_eq!(error.raw_os_error(), Some(32));
}

#[tokio::test]
async fn cleanup_removes_files_and_nested_directories() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("test.db");
    std::fs::write(&file, b"temporary test file").unwrap();
    remove_file(&file).await.unwrap();
    assert!(!file.exists());
    assert_eq!(
        remove_file(&file).await.unwrap_err().kind(),
        io::ErrorKind::NotFound
    );

    let nested = directory.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("test.db"), b"temporary test file").unwrap();
    remove_dir_all(&nested).await.unwrap();
    assert!(!nested.exists());
    directory.close().unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn cleanup_waits_for_real_windows_handles_to_close() {
    use std::os::windows::fs::OpenOptionsExt;

    for whole_directory in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("database");
        std::fs::create_dir(&nested).unwrap();
        let file = nested.join("locked.db");
        std::fs::write(&file, b"temporary test file").unwrap();
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .unwrap();
        assert_eq!(
            std::fs::remove_file(&file).unwrap_err().raw_os_error(),
            Some(32)
        );
        let release = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            drop(handle);
        });
        if whole_directory {
            remove_dir_all(&nested).await.unwrap();
        } else {
            remove_file(&file).await.unwrap();
        }
        release.await.unwrap();
        assert!(!file.exists());
        directory.close().unwrap();
    }
}
