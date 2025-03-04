use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, with_boxed_database,
};
use testkit_postgres::{
    PostgresBackend, PostgresConnection, PostgresError, postgres_backend_with_config,
};

// Helper function to create a test config with the correct hostname
fn test_config() -> DatabaseConfig {
    // Use "postgres" as the hostname
    let admin_url = "postgres://postgres:postgres@postgres:5432/postgres";
    let user_url = "postgres://postgres:postgres@postgres:5432/postgres";
    DatabaseConfig::new(admin_url, user_url)
}

// Helper to create a test backend with config
async fn test_backend_with_config() -> Result<PostgresBackend, PostgresError> {
    let config = test_config();
    postgres_backend_with_config(config).await
}

#[tokio::test]
async fn test_postgres_backend_creation() {
    // Create a backend with the test config
    let result = test_backend_with_config().await;

    // Assert that the backend creation succeeded
    assert!(result.is_ok(), "Failed to create PostgreSQL backend");

    // Get the backend instance
    let backend = result.unwrap();

    // Check that the backend has the expected connection string
    let db_name = DatabaseName::new(None);
    let conn_string = backend.connection_string(&db_name);

    // The connection string should contain the database name
    assert!(
        conn_string.contains(db_name.as_str()),
        "Connection string does not contain the database name"
    );
}

#[tokio::test]
async fn test_database_creation() {
    // Create a backend
    let backend = test_backend_with_config()
        .await
        .expect("Failed to create backend");

    // Create a database using the boxed database API
    let ctx = with_boxed_database(backend)
        .execute()
        .await
        .expect("Failed to create database");

    // Acquire a connection to verify the database was created
    let conn_result = ctx.db.pool.acquire().await;

    // Assert that we can acquire a connection
    assert!(
        conn_result.is_ok(),
        "Failed to acquire connection to the created database"
    );

    // Check the database name in the connection string
    let pool_conn_string = ctx.db.pool.connection_string();
    assert!(
        pool_conn_string.contains("postgres"),
        "Connection string does not contain expected database name"
    );
}

#[tokio::test]
async fn test_simple_query() {
    // Create a backend
    let backend = test_backend_with_config()
        .await
        .expect("Failed to create backend");

    // Create a database with a setup function that creates a test table
    let ctx = with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a test table
                conn.client()
                    .execute(
                        "CREATE TABLE simple_test (id SERIAL PRIMARY KEY, value TEXT NOT NULL)",
                        &[],
                    )
                    .await?;

                // Insert a test row
                conn.client()
                    .execute(
                        "INSERT INTO simple_test (value) VALUES ($1)",
                        &[&"test_value"],
                    )
                    .await?;

                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to setup database");

    // Acquire a connection to query the database
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    // Query the test row
    let rows = conn
        .client()
        .query("SELECT value FROM simple_test WHERE id = 1", &[])
        .await
        .expect("Failed to query test table");

    // Assert that we found the expected row and value
    assert_eq!(rows.len(), 1, "Expected 1 row in the result");
    let value: String = rows[0].get(0);
    assert_eq!(value, "test_value", "Expected value to be 'test_value'");
}

#[tokio::test]
async fn test_manual_transaction() {
    // Create a backend with config for the postgres host
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );
    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a database context
    let ctx = with_boxed_database(backend)
        .execute()
        .await
        .expect("Failed to create database context");

    // Acquire a connection
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    // Create a test table
    conn.client()
        .execute(
            "CREATE TABLE test_table (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Begin a transaction
    conn.client()
        .execute("BEGIN", &[])
        .await
        .expect("Failed to begin transaction");

    // Insert a row
    conn.client()
        .execute(
            "INSERT INTO test_table (name) VALUES ('test_transaction')",
            &[],
        )
        .await
        .expect("Failed to insert data");

    // Query the table within the transaction
    let rows = conn
        .client()
        .query("SELECT * FROM test_table", &[])
        .await
        .expect("Failed to query table");

    // Verify the data exists in the transaction
    assert_eq!(rows.len(), 1, "Expected 1 row in transaction");
    let name: String = rows[0].get("name");
    assert_eq!(
        name, "test_transaction",
        "Expected name to be 'test_transaction'"
    );

    // Commit the transaction
    conn.client()
        .execute("COMMIT", &[])
        .await
        .expect("Failed to commit transaction");

    // Query again to verify the data persists after commit
    let rows = conn
        .client()
        .query("SELECT * FROM test_table", &[])
        .await
        .expect("Failed to query table after commit");

    assert_eq!(rows.len(), 1, "Expected 1 row after commit");
}

// Define handler structs to avoid lifetime issues
#[allow(dead_code)]
struct SetupHandler;
#[allow(dead_code)]
struct TransactionHandler;

#[tokio::test]
async fn test_transaction_api() {
    // Create a backend with config for the postgres host
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );
    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a database context
    let ctx = with_boxed_database(backend)
        .execute()
        .await
        .expect("Failed to create database context");

    // Create a test table
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");
    conn.client()
        .execute(
            "CREATE TABLE transaction_test (id SERIAL PRIMARY KEY, value TEXT NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Get a connection for the transaction
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection for transaction");

    // Begin a transaction
    conn.client()
        .execute("BEGIN", &[])
        .await
        .expect("Failed to begin transaction");

    // Insert a row
    conn.client()
        .execute(
            "INSERT INTO transaction_test (value) VALUES ('test_value')",
            &[],
        )
        .await
        .expect("Failed to insert data");

    // Commit the transaction
    conn.client()
        .execute("COMMIT", &[])
        .await
        .expect("Failed to commit transaction");

    // Query to verify data exists
    let rows = conn
        .client()
        .query("SELECT * FROM transaction_test", &[])
        .await
        .expect("Failed to query table");

    // Verify the data
    assert_eq!(rows.len(), 1, "Expected 1 row");
    let value: String = rows[0].get("value");
    assert_eq!(value, "test_value", "Expected value to be 'test_value'");
}

#[tokio::test]
async fn test_transaction_rollback() {
    // Create a backend with config for the postgres host
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );
    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a database context using the boxed API
    let ctx = with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a test table
                conn.client()
                    .execute(
                        "CREATE TABLE rollback_test (id SERIAL PRIMARY KEY, value TEXT NOT NULL)",
                        &[],
                    )
                    .await?;
                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to create database context");

    // Get a connection for the transaction
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    // Begin a transaction
    conn.client()
        .execute("BEGIN", &[])
        .await
        .expect("Failed to begin transaction");

    // Insert a row
    conn.client()
        .execute(
            "INSERT INTO rollback_test (value) VALUES ('will_be_rolled_back')",
            &[],
        )
        .await
        .expect("Failed to insert data");

    // Verify the data exists within the transaction
    let rows = conn
        .client()
        .query("SELECT * FROM rollback_test", &[])
        .await
        .expect("Failed to query table within transaction");
    assert_eq!(rows.len(), 1, "Expected 1 row within transaction");

    // Rollback the transaction
    conn.client()
        .execute("ROLLBACK", &[])
        .await
        .expect("Failed to rollback transaction");

    // Verify the data no longer exists
    let rows = conn
        .client()
        .query("SELECT * FROM rollback_test", &[])
        .await
        .expect("Failed to query table after rollback");
    assert_eq!(rows.len(), 0, "Expected 0 rows after rollback");
}

#[tokio::test]
async fn test_transaction_error_handling() {
    // Create a backend with config for the postgres host
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );
    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a database context using the boxed API
    let ctx = with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a test table with a unique constraint
                conn.client()
                    .execute(
                        "CREATE TABLE error_test (
                        id SERIAL PRIMARY KEY, 
                        value TEXT NOT NULL,
                        CONSTRAINT unique_value UNIQUE (value)
                    )",
                        &[],
                    )
                    .await?;

                // Insert initial data
                conn.client()
                    .execute(
                        "INSERT INTO error_test (value) VALUES ('unique_value')",
                        &[],
                    )
                    .await?;

                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to create database context");

    // Get a connection for the transaction
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    // Begin a transaction
    conn.client()
        .execute("BEGIN", &[])
        .await
        .expect("Failed to begin transaction");

    // Try to insert data that violates the unique constraint
    let result = conn
        .client()
        .execute(
            "INSERT INTO error_test (value) VALUES ('unique_value')",
            &[],
        )
        .await;

    // Verify the insert failed due to the constraint
    assert!(
        result.is_err(),
        "Insert should have failed due to unique constraint"
    );
    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("unique") || error.to_string().contains("duplicate"),
        "Error should mention uniqueness violation"
    );

    // Rollback the transaction
    conn.client()
        .execute("ROLLBACK", &[])
        .await
        .expect("Failed to rollback transaction");

    // Verify we only have one row (the original insert)
    let rows = conn
        .client()
        .query("SELECT * FROM error_test", &[])
        .await
        .expect("Failed to query table after error");
    assert_eq!(rows.len(), 1, "Expected only 1 row (the original insert)");
}

// Handler for high-level transaction API testing
struct HighLevelTransactionHandler;

impl HighLevelTransactionHandler {
    async fn run_transaction(
        &self,
        conn: &PostgresConnection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Begin the transaction
        conn.client().execute("BEGIN", &[]).await?;

        // Create a table if it doesn't exist
        conn.client()
            .execute(
                "CREATE TABLE IF NOT EXISTS high_level_test (
                    id SERIAL PRIMARY KEY, 
                    value TEXT NOT NULL
                )",
                &[],
            )
            .await?;

        // Insert data
        conn.client()
            .execute(
                "INSERT INTO high_level_test (value) VALUES ($1)",
                &[&"high_level_value"],
            )
            .await?;

        // Commit the transaction
        conn.client().execute("COMMIT", &[]).await?;

        Ok(())
    }

    async fn run_failing_transaction(
        &self,
        conn: &PostgresConnection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Begin the transaction
        conn.client().execute("BEGIN", &[]).await?;

        // Try to query a non-existent table (will fail)
        let result = conn
            .client()
            .query("SELECT * FROM non_existent_table", &[])
            .await;

        if result.is_err() {
            // Try to roll back the transaction if the query fails,
            // but still return the original error
            let _ = conn.client().execute("ROLLBACK", &[]).await;
            return Err(result.err().unwrap().into());
        }

        // This won't be reached if the above query fails
        conn.client().execute("COMMIT", &[]).await?;

        Ok(())
    }
}

#[tokio::test]
async fn test_high_level_transaction_api() {
    // Create a backend with config for the postgres host
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );
    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Create a database context
    let ctx = with_boxed_database(backend)
        .execute()
        .await
        .expect("Failed to create database context");

    // Run successful transaction
    let handler = HighLevelTransactionHandler {};
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    let result = handler.run_transaction(&conn).await;
    assert!(result.is_ok(), "Transaction should succeed: {:?}", result);

    // Verify the data was inserted
    let rows = conn
        .client()
        .query("SELECT * FROM high_level_test", &[])
        .await
        .expect("Failed to query table after successful transaction");
    assert_eq!(rows.len(), 1, "Expected 1 row after successful transaction");
    let value: String = rows[0].get("value");
    assert_eq!(
        value, "high_level_value",
        "Expected value to be 'high_level_value'"
    );

    // Run failing transaction
    let result = handler.run_failing_transaction(&conn).await;
    assert!(result.is_err(), "Transaction should fail");

    // Explicitly roll back the transaction after the failure
    // This is necessary because PostgreSQL requires an explicit ROLLBACK
    // after a transaction fails before the connection can be used again
    conn.client()
        .execute("ROLLBACK", &[])
        .await
        .expect("Failed to roll back failed transaction");

    // Verify the transaction was rolled back
    // The previous data should still be there (1 row)
    let rows = conn
        .client()
        .query("SELECT * FROM high_level_test", &[])
        .await
        .expect("Failed to query table after failed transaction");
    assert_eq!(
        rows.len(),
        1,
        "Still expected 1 row after failed transaction"
    );
}

#[tokio::test]
async fn test_boxed_transaction() {
    // Create a backend with config
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@postgres:5432/postgres",
        "postgres://postgres:postgres@postgres:5432/postgres",
    );
    let backend = postgres_backend_with_config(config)
        .await
        .expect("Failed to create backend");

    // Local variables to capture
    let table_name = String::from("boxed_transaction_test");
    let test_value = String::from("test value");

    // Clone variables for both functions
    let table_name_setup = table_name.clone();
    let table_name_tx = table_name.clone();
    let test_value_tx = test_value.clone();

    // Use boxed database API
    let ctx = with_boxed_database(backend)
        .setup(|conn| {
            Box::pin(async move {
                // Create a table using the captured variable
                let create_query = format!(
                    "CREATE TABLE {} (id SERIAL PRIMARY KEY, value TEXT NOT NULL)",
                    table_name_setup
                );
                conn.client().execute(&create_query, &[]).await?;
                Ok(())
            })
        })
        .with_transaction(|conn| {
            Box::pin(async move {
                // Insert data using the captured variables
                let insert_query = format!("INSERT INTO {} (value) VALUES ($1)", table_name_tx);
                conn.client()
                    .execute(&insert_query, &[&test_value_tx])
                    .await?;
                Ok(())
            })
        })
        .execute()
        .await
        .expect("Failed to setup database with transaction");

    // Verify data exists
    let conn = ctx.db.pool.acquire().await.unwrap();
    let query = format!("SELECT * FROM {} WHERE value = $1", table_name);
    let rows = conn.client().query(&query, &[&test_value]).await.unwrap();

    assert_eq!(rows.len(), 1, "Expected 1 row in the table");
    let value: String = rows[0].get("value");
    assert_eq!(value, test_value, "Expected value to match test_value");
}
