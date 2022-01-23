#![feature(type_alias_impl_trait)]

use sqlx::sqlite::SqlitePool;
use warp::Filter;

mod handlers;

async fn connect_to_db() -> SqlitePool {
    SqlitePool::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set").as_str())
        .await
        .expect("Database pool creation failed.")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let content_dir = std::env::args().nth(1).expect("Usage: ./server content/");
    
    let pool = connect_to_db().await;
    
    let route = warp::path("static")
        .and(warp::fs::dir(content_dir.clone()))
        .or(handlers::filters(pool, content_dir));

    warp::serve(route).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}