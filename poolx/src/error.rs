use std::error::Error as StdError;

pub type BoxDynError = Box<dyn StdError + 'static + Send + Sync>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Error occurred while parsing a connection string.
    #[error("error with configuration: {0}")]
    Configuration(#[source] BoxDynError),
    /// Error communicating with the database backend.
    #[error("error communicating with database: {0}")]
    Io(#[from] std::io::Error),

    /// A [`Pool::acquire`] timed out due to connections not becoming available or
    /// because another task encountered too many errors while trying to open a new connection.
    ///
    /// [`Pool::acquire`]: crate::pool::Pool::acquire
    #[error("pool timed out while waiting for an open connection")]
    PoolTimedOut,

    /// [`Pool::close`] was called while we were waiting in [`Pool::acquire`].
    ///
    /// [`Pool::acquire`]: crate::pool::Pool::acquire
    /// [`Pool::close`]: crate::pool::Pool::close
    #[error("attempted to acquire a connection on a closed pool")]
    PoolClosed,

    #[error("error response from server")]
    ResponseError,
}
