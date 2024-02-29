use std::io::ErrorKind;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use futures_core::future::BoxFuture;
use redis::aio::ConnectionLike;
use redis::Client;

use ypool::{Connection, ConnectOptions, futures_core, url};
use ypool::url::Url;

#[derive(Debug, Clone)]
pub struct RedisConnectionOption {
    url: Url,
    client: Client,
}

impl RedisConnectionOption {}

impl FromStr for RedisConnectionOption {
    type Err = ypool::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = s.parse::<Url>().map_err(|e| ypool::Error::Configuration(Box::new(e)))?;
        Self::from_url(&url)
    }
}

impl ConnectOptions for RedisConnectionOption {
    type Connection = RedisConnection;

    fn from_url(url: &url::Url) -> Result<Self, ypool::Error> {
        let client = Client::open(url.clone()).map_err(|e| ypool::Error::Configuration(Box::new(e)))?;
        Ok(Self {
            url: url.clone(),
            client,
        })
    }

    fn connect(&self) -> BoxFuture<'_, Result<Self::Connection, ypool::Error>> where Self::Connection: Sized {
        Box::pin(async move {
            let conn = self.client.get_async_connection().await.map_err(|e| ypool::Error::Io(std::io::Error::from(ErrorKind::ConnectionReset)))?;
            Ok(RedisConnection { inner: conn })
        })
    }
}

pub struct RedisConnection {
    pub inner: redis::aio::Connection,
}

impl AsMut<redis::aio::Connection> for RedisConnection {
    fn as_mut(&mut self) -> &mut redis::aio::Connection {
        &mut self.inner
    }
}


impl Connection for RedisConnection {
    type Options = RedisConnectionOption;

    fn close(mut self) -> BoxFuture<'static, Result<(), ypool::Error>> {
        Box::pin(async move {
            self.inner.req_packed_command(&redis::cmd("QUIT")).await.map_err(|e| std::io::Error::new(ErrorKind::ConnectionReset, e.to_string()))?;
            Ok(())
        })
    }

    fn close_hard(self) -> BoxFuture<'static, Result<(), ypool::Error>> {
        Box::pin(async move {
            Ok(())
        })
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), ypool::Error>> {
        Box::pin(async move {
            let pong: String = redis::cmd("PING").query_async(&mut self.inner).await.map_err(|e| std::io::Error::new(ErrorKind::ConnectionReset, e.to_string()))?;
            match pong.as_str() {
                "PONG" => Ok(()),
                _ => Err(ypool::Error::ResponseError),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use redis::cmd;

    use ypool::{Pool, PoolOptions};

    use crate::RedisConnection;

    #[tokio::test]
    async fn test_redis_connection_option_from_str() {
        let url = "redis://:foobared@127.0.0.1:6379";
        let option = url.parse::<super::RedisConnectionOption>().unwrap();

        let pool: Pool<RedisConnection> = PoolOptions::new()
            .test_before_acquire(true)
            .idle_timeout(std::time::Duration::from_secs(3))
            .min_connections(3)
            .max_connections(100)
            .connect_lazy_with(option);

        for i in 0..10 {
            let mut conn = pool.acquire().await.unwrap();
            let reply: String = cmd("PING").query_async(&mut conn.inner).await.unwrap();
            println!("reply: {}", reply);
        }
    }
}
