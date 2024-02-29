use std::fmt::Debug;
use std::str::FromStr;
use futures_core::future::BoxFuture;
use url::Url;
use crate::database::Database;
use crate::error::Error;

pub trait Connection: Send {
    type Database: Database;

    type Options: ConnectOptions<Connection=Self>;

    /// Explicitly close this database connection.
    ///
    /// This notifies the database server that the connection is closing so that it can
    /// free up any server-side resources in use.
    ///
    /// While connections can simply be dropped to clean up local resources,
    /// the `Drop` handler itself cannot notify the server that the connection is being closed
    /// because that may require I/O to send a termination message. That can result in a delay
    /// before the server learns that the connection is gone, usually from a TCP keepalive timeout.
    ///
    /// Creating and dropping many connections in short order without calling `.close()` may
    /// lead to errors from the database server because those senescent connections will still
    /// count against any connection limit or quota that is configured.
    ///
    /// Therefore it is recommended to call `.close()` on a connection when you are done using it
    /// and to `.await` the result to ensure the termination message is sent.
    fn close(self) -> BoxFuture<'static, Result<(), Error>>;

    fn close_hard(self) -> BoxFuture<'static, Result<(), Error>>;
    fn ping(&mut self) -> BoxFuture<'_, Result<(), Error>>;

}

pub trait ConnectOptions: 'static + Send + Sync + FromStr<Err = Error> + Debug + Clone {
    type Connection: Connection + ?Sized;

    /// Parse the `ConnectOptions` from a URL.
    fn from_url(url: &Url) -> Result<Self, Error>;

    /// Establish a new database connection with the options specified by `self`.
    fn connect(&self) -> BoxFuture<'_, Result<Self::Connection, Error>>
        where
            Self::Connection: Sized;


}