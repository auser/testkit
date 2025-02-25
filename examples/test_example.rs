#[tokio::test]
async fn test_get_organization() {
    with_test_db!(
        PostgresPool,
        |mut conn| async move {
            // Setup the database
            conn.execute("CREATE TABLE organizations (id SERIAL PRIMARY KEY, name TEXT)")
                .await?;
            conn.execute("INSERT INTO organizations (name) VALUES ('Test Organization')")
                .await?;
            Ok(())
        },
        |db| async move {
            // Get a connection from the pool
            let mut conn = db.test_pool.acquire().await.unwrap();
            let org = get_organization(&mut conn).await;
            assert!(org.is_ok());

            // Test with a transaction
            let mut tx = conn.begin().await.unwrap();
            tx.execute("INSERT INTO organizations (name) VALUES ('Another Org')")
                .await
                .unwrap();

            // We can roll back or commit
            tx.commit().await.unwrap();
        }
    )
    .await;
    // Database will be automatically dropped here
}

fn main() {
    // This example is meant to be run with cargo test
}
