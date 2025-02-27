#[allow(unused_imports)]
use db_testkit::prelude::*;

#[tokio::test]
async fn test_with_custom_connection() {
    with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres",
        |conn| async move {
            // Setup code
            conn.execute("CREATE TABLE test (id SERIAL PRIMARY KEY)")
                .await
                .unwrap();
            Ok(())
        },
        |db| async move {
            // Test code
            let mut conn = db.connection().await.unwrap();
            conn.execute("INSERT INTO test (id) VALUES (1)")
                .await
                .unwrap();
        }
    );
}

fn main() {
    // This example is meant to be run with cargo test
}
