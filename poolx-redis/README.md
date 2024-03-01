Poolx is a generic connection pool implementation for Rust, Its original code is from sqlx, and I have made many changes
to make it more generic and remove lots of useless code.

## features

- test on borrow
- idle connection with timeout check
- customize close/ping method implementation
- lazy connection

## example usage

```rust
#[tokio::main]
async fn main() {
    let url = "redis://:foobared@127.0.0.1:6379";
    let option = url.parse::<RedisConnectionOption>().unwrap();

    let pool: Pool<RedisConnection> = PoolOptions::new()
        .test_before_acquire(true)
        .idle_timeout(std::time::Duration::from_secs(3))
        .min_connections(3)
        .max_connections(100)
        .connect_lazy_with(option);

    for i in 0..10 {
        let mut conn = pool.acquire().await.unwrap();
        let reply: String = cmd("PING").query_async(conn.as_mut()).await.unwrap();
        println!("reply: {}", reply);
    }
}
```