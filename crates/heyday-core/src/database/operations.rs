//! One process-wide gate for database operations and exclusive restore.
//! Application commands retain the shared guard for their full operation.
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

static DATABASE_OPERATIONS: RwLock<()> = RwLock::const_new(());

pub fn operation() -> Result<RwLockReadGuard<'static, ()>, String> {
    DATABASE_OPERATIONS.try_read().map_err(|_| {
        "Database restore is in progress. Wait for the app to reload before trying again.".into()
    })
}

pub async fn exclusive() -> RwLockWriteGuard<'static, ()> {
    DATABASE_OPERATIONS.write().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::{poll_fn, Future},
        task::Poll,
    };

    #[tokio::test]
    async fn queued_restore_rejects_new_operations_and_waits_for_current_readers() {
        let reader = operation().unwrap();
        let mut writer = Box::pin(exclusive());
        poll_fn(|cx| {
            assert!(writer.as_mut().poll(cx).is_pending());
            // Tokio's queued writer blocks new shared guards before acquiring exclusivity.
            assert!(operation().is_err());
            Poll::Ready(())
        })
        .await;
        drop(reader);
        let exclusive = writer.await;
        assert!(operation().is_err());
        drop(exclusive);
        assert!(operation().is_ok());
    }
}
