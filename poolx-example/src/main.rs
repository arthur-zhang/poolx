use std::fmt::Debug;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;

use tokio::net::TcpStream;

use poolx::{Connection, ConnectOptions, Error, Pool, PoolOptions};
use poolx::futures_core::future::BoxFuture;
use poolx::url::Url;

pub struct MyConn {
    id: u64,
    inner: TcpStream,
}

impl Connection for MyConn {
    type Options = MyConnOption;

    fn close(mut self) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            Ok(())
        })
    }

    fn close_hard(mut self) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            self.inner.shutdown().await?;
            Ok(())
        })
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            Ok(())
        })
    }
}


#[derive(Debug)]
pub struct MyConnOption {
    addr: SocketAddr,
    counter: AtomicU64,
}

impl Clone for MyConnOption {
    fn clone(&self) -> Self {
        MyConnOption {
            addr: self.addr,
            counter: Default::default(),
        }
    }
}

impl FromStr for MyConnOption {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = s.parse::<Url>().map_err(|e| Error::Configuration(Box::new(e)))?;
        Self::from_url(&url)
    }
}


impl ConnectOptions for MyConnOption {
    type Connection = MyConn;

    fn from_url(url: &Url) -> Result<Self, Error> {
        let addr = url.host_str().unwrap();
        let port = url.port().unwrap();
        let addr = format!("{}:{}", addr, port);

        Ok(MyConnOption {
            counter: AtomicU64::new(0),
            addr: addr.parse().unwrap(),
        })
    }

    fn connect(&self) -> BoxFuture<'_, Result<Self::Connection, Error>> where Self::Connection: Sized {
        Box::pin(async move {
            let conn = TcpStream::connect(self.addr).await?;
            Ok(MyConn { id: self.counter.fetch_add(1, Ordering::Relaxed), inner: conn })
        })
    }
}


#[tokio::main]
async fn main() {
    let conn_option = "tcp://127.0.0.1:6379".parse::<MyConnOption>().unwrap();
    let pool: Pool<MyConn> = PoolOptions::new()
        .idle_timeout(std::time::Duration::from_secs(3))
        .min_connections(3)
        .max_connections(100)
        .connect_lazy_with(conn_option);

    let mut vec = vec![];
    for _i in 0..10 {
        let conn = pool.acquire().await.unwrap();
        println!("conn: {}", conn.id);
        vec.push(conn);
    }
    println!("release 10 connections");
    for _i in 0..10 {
        vec.pop();
    }
    tokio::time::sleep(tokio::time::Duration::from_secs(10000)).await;
}