#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Error occurred while parsing a connection string.

    /// Error communicating with the database backend.
    #[error("error communicating with database: {0}")]
    Io(#[from] std::io::Error),
    /// Unexpected or invalid data encountered while communicating with the database.
    ///
    /// This should indicate there is a programming error in a SQLx driver or there
    /// is something corrupted with the connection to the database itself.
    #[error("encountered unexpected or invalid data: {0}")]
    Protocol(String),

    /// No rows returned by a query that expected to return at least one row.
    #[error("no rows returned by a query that expected to return at least one row")]
    RowNotFound,

    /// Type in query doesn't exist. Likely due to typo or missing user type.
    #[error("type named {type_name} not found")]
    TypeNotFound { type_name: String },

    /// Column index was out of bounds.
    #[error("column index out of bounds: the len is {len}, but the index is {index}")]
    ColumnIndexOutOfBounds { index: usize, len: usize },

    /// No column found for the given name.
    #[error("no column found for name: {0}")]
    ColumnNotFound(String),



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

    /// A background worker has crashed.
    #[error("attempted to communicate with a crashed background worker")]
    WorkerCrashed,

}
