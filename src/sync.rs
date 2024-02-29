pub struct AsyncSemaphoreReleaser<'a> {
    inner: tokio::sync::SemaphorePermit<'a>,
}

impl AsyncSemaphoreReleaser<'_> {
    pub fn disarm(self) {
        self.inner.forget();
        return;
    }
}


pub struct AsyncSemaphore {
    inner: tokio::sync::Semaphore,
}

impl AsyncSemaphore {
    pub fn new(fair: bool, permits: usize) -> Self {
        AsyncSemaphore {
            inner: {
                tokio::sync::Semaphore::new(permits)
            },
        }
    }

    pub fn permits(&self) -> usize {
        return self.inner.available_permits();
    }

    pub async fn acquire(&self, permits: u32) -> AsyncSemaphoreReleaser<'_> {
        return AsyncSemaphoreReleaser {
            inner: self
                .inner
                // Weird quirk: `tokio::sync::Semaphore` mostly uses `usize` for permit counts,
                // but `u32` for this and `try_acquire_many()`.
                .acquire_many(permits)
                .await
                .expect("BUG: we do not expose the `.close()` method"),
        };
    }

    pub fn try_acquire(&self, permits: u32) -> Option<AsyncSemaphoreReleaser<'_>> {
        return Some(AsyncSemaphoreReleaser {
            inner: self.inner.try_acquire_many(permits).ok()?,
        });
    }

    pub fn release(&self, permits: usize) {
        return self.inner.add_permits(permits);
    }
}