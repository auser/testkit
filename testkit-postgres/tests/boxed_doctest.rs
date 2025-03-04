use testkit_core::boxed_async;
use testkit_core::with_boxed_database;
use testkit_postgres::postgres_backend;

/// Test that replicates the boxed example doctest
#[tokio::test]
async fn test_boxed_api_example() {
    // Skip the test if we can't connect to the database
    let backend = match postgres_backend().await {
        Ok(backend) => backend,
        Err(_) => return, // Skip test if we can't connect to the database
    };

    // Create a table name that we'll need to capture in both closures
    let table_name = String::from("users_test");
    let table_name_for_transaction = table_name.clone(); // Clone for the second closure

    // Use our boxed_async macro to simplify the code
    let ctx = with_boxed_database(backend)
        .setup(move |conn| {
            boxed_async!(async move {
                // In a real scenario, this would create a table
                let query = format!(
                    "CREATE TABLE IF NOT EXISTS {} (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL
            )",
                    table_name
                );

                // Execute the query using the PostgresConnection trait methods
                let _result = conn.client().execute(&query, &[]).await?;
                Ok(())
            })
        })
        .with_transaction(move |conn| {
            boxed_async!(async move {
                // This would be the transaction, now using the cloned table name
                let query = format!(
                    "INSERT INTO {} (name) VALUES ($1)",
                    table_name_for_transaction
                );
                let _result = conn.client().execute(&query, &[&"test_user"]).await?;
                Ok(())
            })
        })
        .execute()
        .await;

    assert!(
        ctx.is_ok(),
        "Failed to execute boxed database example: {:?}",
        ctx.err()
    );
}
