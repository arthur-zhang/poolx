use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::conn::{Connection, ConnectOptions};
use crate::error::Error;
use crate::PoolConnectionMetadata;
use crate::sync::AsyncSemaphoreReleaser;

use super::inner::{DecrementSizeGuard, PoolInner};

/// A connection managed by a [`Pool`][crate::pool::Pool].
///
/// Will be returned to the pool on-drop.
pub struct PoolConnection<C: Connection> {
    live: Option<Live<C>>,
    pub(crate) pool: Arc<PoolInner<C>>,
}

pub(super) struct Live<C: Connection> {
    pub(super) raw: C,
    pub(super) created_at: Instant,
}

pub(super) struct Idle<C: Connection> {
    pub(super) live: Live<C>,
    pub(super) idle_since: Instant,
}

/// RAII wrapper for connections being handled by functions that may drop them
pub(super) struct Floating<Conn: Connection, C> {
    pub(super) inner: C,
    pub(super) guard: DecrementSizeGuard<Conn>,
}

const EXPECT_MSG: &str = "BUG: inner connection already taken!";

impl<C: Connection> Debug for PoolConnection<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // TODO: Show the type name of the connection ?
        f.debug_struct("PoolConnection").finish()
    }
}

impl<C: Connection> Deref for PoolConnection<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.live.as_ref().expect(EXPECT_MSG).raw
    }
}

impl<C: Connection> DerefMut for PoolConnection<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.live.as_mut().expect(EXPECT_MSG).raw
    }
}

impl<C: Connection> AsRef<C> for PoolConnection<C> {
    fn as_ref(&self) -> &C {
        self
    }
}

impl<C: Connection> AsMut<C> for PoolConnection<C> {
    fn as_mut(&mut self) -> &mut C {
        self
    }
}

impl<C: Connection> PoolConnection<C> {
    /// Close this connection, allowing the pool to open a replacement.
    ///
    /// Equivalent to calling [`.detach()`] then [`.close()`], but the connection permit is retained
    /// for the duration so that the pool may not exceed `max_connections`.
    ///
    /// [`.detach()`]: PoolConnection::detach
    /// [`.close()`]: Connection::close
    pub async fn close(mut self) -> Result<(), Error> {
        let floating = self.take_live().float(self.pool.clone());
        floating.inner.raw.close().await
    }

    /// Detach this connection from the pool, allowing it to open a replacement.
    ///
    /// Note that if your application uses a single shared pool, this
    /// effectively lets the application exceed the [`max_connections`] setting.
    ///
    /// If [`min_connections`] is nonzero, a task will be spawned to replace this connection.
    ///
    /// If you want the pool to treat this connection as permanently checked-out,
    /// use [`.leak()`][Self::leak] instead.
    ///
    /// [`max_connections`]: crate::pool::PoolOptions::max_connections
    /// [`min_connections`]: crate::pool::PoolOptions::min_connections
    pub fn detach(mut self) -> C {
        self.take_live().float(self.pool.clone()).detach()
    }

    /// Detach this connection from the pool, treating it as permanently checked-out.
    ///
    /// This effectively will reduce the maximum capacity of the pool by 1 every time it is used.
    ///
    /// If you don't want to impact the pool's capacity, use [`.detach()`][Self::detach] instead.
    pub fn leak(mut self) -> C {
        self.take_live().raw
    }

    fn take_live(&mut self) -> Live<C> {
        self.live.take().expect(EXPECT_MSG)
    }

    /// Test the connection to make sure it is still live before returning it to the pool.
    ///
    /// This effectively runs the drop handler eagerly instead of spawning a task to do it.
    #[doc(hidden)]
    pub fn return_to_pool(&mut self) -> impl Future<Output=()> + Send + 'static {
        // float the connection in the pool before we move into the task
        // in case the returned `Future` isn't executed, like if it's spawned into a dying runtime
        // https://github.com/launchbadge/sqlx/issues/1396
        // Type hints seem to be broken by `Option` combinators in IntelliJ Rust right now (6/22).
        let floating: Option<Floating<C, Live<C>>> =
            self.live.take().map(|live| live.float(self.pool.clone()));

        let pool = self.pool.clone();

        async move {
            let returned_to_pool = if let Some(floating) = floating {
                floating.return_to_pool().await
            } else {
                false
            };

            if !returned_to_pool {
                pool.min_connections_maintenance(None).await;
            }
        }
    }
}


/// Returns the connection to the [`Pool`][crate::pool::Pool] it was checked-out from.
impl<C: Connection> Drop for PoolConnection<C> {
    fn drop(&mut self) {
        // We still need to spawn a task to maintain `min_connections`.
        if self.live.is_some() || self.pool.options.min_connections > 0 {
            tokio::spawn(self.return_to_pool());
        }
    }
}

impl<C: Connection> Live<C> {
    pub fn float(self, pool: Arc<PoolInner<C>>) -> Floating<C, Self> {
        Floating {
            inner: self,
            // create a new guard from a previously leaked permit
            guard: DecrementSizeGuard::new_permit(pool),
        }
    }

    pub fn into_idle(self) -> Idle<C> {
        Idle {
            live: self,
            idle_since: Instant::now(),
        }
    }
}

impl<C: Connection> Deref for Idle<C> {
    type Target = Live<C>;

    fn deref(&self) -> &Self::Target {
        &self.live
    }
}

impl<C: Connection> DerefMut for Idle<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.live
    }
}

impl<C: Connection> Floating<C, Live<C>> {
    pub fn new_live(conn: C, guard: DecrementSizeGuard<C>) -> Self {
        Self {
            inner: Live {
                raw: conn,
                created_at: Instant::now(),
            },
            guard,
        }
    }

    pub fn reattach(self) -> PoolConnection<C> {
        let Floating { inner, guard } = self;

        let pool = Arc::clone(&guard.pool);

        guard.cancel();
        PoolConnection {
            live: Some(inner),
            pool,
        }
    }

    pub fn release(self) {
        self.guard.pool.clone().release(self);
    }

    /// Return the connection to the pool.
    ///
    /// Returns `true` if the connection was successfully returned, `false` if it was closed.
    async fn return_to_pool(mut self) -> bool {
        // Immediately close the connection.
        if self.guard.pool.is_closed() {
            self.close().await;
            return false;
        }

        if let Some(test) = &self.guard.pool.options.after_release {
            let meta = self.metadata();
            match (test)(&mut self.inner.raw, meta).await {
                Ok(true) => (),
                Ok(false) => {
                    self.close().await;
                    return false;
                }
                Err(error) => {
                    tracing::warn!(%error, "error from `after_release`");
                    // Connection is broken, don't try to gracefully close as
                    // something weird might happen.
                    self.close_hard().await;
                    return false;
                }
            }
        }

        // test the connection on-release to ensure it is still viable,
        // and flush anything time-sensitive like transaction rollbacks
        // if an Executor future/stream is dropped during an `.await` call, the connection
        // is likely to be left in an inconsistent state, in which case it should not be
        // returned to the pool; also of course, if it was dropped due to an error
        // this is simply a band-aid as SQLx-next connections should be able
        // to recover from cancellations
        // if let Err(error) = self.raw.ping().await {
        //     tracing::warn!(
        //         %error,
        //         "error occurred while testing the connection on-release",
        //     );
        //
        //     // Connection is broken, don't try to gracefully close.
        //     self.close_hard().await;
        //     false
        // } else {
            // if the connection is still viable, release it to the pool
            self.release();
            true
        // }
    }

    pub async fn close(self) {
        // This isn't used anywhere that we care about the return value
        let _ = self.inner.raw.close().await;

        // `guard` is dropped as intended
    }

    pub async fn close_hard(self) {
        let _ = self.inner.raw.close_hard().await;
    }

    pub fn detach(self) -> C {
        self.inner.raw
    }

    pub fn into_idle(self) -> Floating<C, Idle<C>> {
        Floating {
            inner: self.inner.into_idle(),
            guard: self.guard,
        }
    }

    pub fn metadata(&self) -> PoolConnectionMetadata {
        PoolConnectionMetadata {
            age: self.created_at.elapsed(),
            idle_for: Duration::ZERO,
        }
    }
}

impl<C: Connection> Floating<C, Idle<C>> {
    pub fn from_idle(
        idle: Idle<C>,
        pool: Arc<PoolInner<C>>,
        permit: AsyncSemaphoreReleaser<'_>,
    ) -> Self {
        Self {
            inner: idle,
            guard: DecrementSizeGuard::from_permit(pool, permit),
        }
    }

    pub async fn ping(&mut self) -> Result<(), Error> {
        self.live.raw.ping().await
    }

    pub fn into_live(self) -> Floating<C, Live<C>> {
        Floating {
            inner: self.inner.live,
            guard: self.guard,
        }
    }

    pub async fn close(self) -> DecrementSizeGuard<C> {
        if let Err(error) = self.inner.live.raw.close().await {
            tracing::debug!(%error, "error occurred while closing the pool connection");
        }
        self.guard
    }

    pub async fn close_hard(self) -> DecrementSizeGuard<C> {
        let _ = self.inner.live.raw.close_hard().await;

        self.guard
    }

    pub fn metadata(&self) -> PoolConnectionMetadata {
        // Use a single `now` value for consistency.
        let now = Instant::now();

        PoolConnectionMetadata {
            // NOTE: the receiver is the later `Instant` and the arg is the earlier
            // https://github.com/launchbadge/sqlx/issues/1912
            age: now.saturating_duration_since(self.created_at),
            idle_for: now.saturating_duration_since(self.idle_since),
        }
    }
}

impl<Conn: Connection, C> Deref for Floating<Conn, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<Conn: Connection, C> DerefMut for Floating<Conn, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
