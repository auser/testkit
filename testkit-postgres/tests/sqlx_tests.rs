#![allow(clippy::all, unused_must_use, unused_lifetimes)]
#![cfg(feature = "with-sqlx")] // This file is specific to sqlx backend

use sqlx::Row;
use std::future::Future;
use std::pin::Pin;
use testkit_core::{DatabaseBackend, TestDatabaseInstance};
use testkit_core::{DatabaseConfig, DatabasePool, boxed_async, with_boxed_database};
use testkit_postgres::{PostgresError, SqlxConnection, TransactionManager};
use testkit_postgres::{
    SqlxPostgresBackend as PostgresBackend,
    sqlx_postgres_backend_with_config as postgres_backend_with_config,
};

// Helper function to create a test config with the correct hostname
fn test_config() -> DatabaseConfig {
    // Use postgres instead of postgres hostname
    let admin_url = "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";
    let user_url = "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";
    DatabaseConfig::new(admin_url, user_url)
}

// Helper function to create a backend with the test config
async fn test_backend() -> Result<PostgresBackend, PostgresError> {
    postgres_backend_with_config(test_config()).await
}

// Helper function to check if an error is a connection error
fn is_connection_error(err: &PostgresError) -> bool {
    let err_str = err.to_string();
    err_str.contains("connection refused")
        || err_str.contains("timeout")
        || err_str.contains("does not exist")
        || err_str.contains("pool timed out")
}

// Box a future for use with the boxed API
#[allow(dead_code)]
fn boxed_future<T, F, Fut>(
    f: F,
) -> impl FnOnce(T) -> Pin<Box<dyn Future<Output = Result<(), PostgresError>> + Send>>
where
    F: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), PostgresError>> + Send + 'static,
    T: Send + 'static,
{
    move |t| Box::pin(f(t))
}

#[tokio::test]
async fn test_sqlx_backend() {
    // Test that we can create a backend using the sqlx library
    let result = test_backend().await;
    if let Err(e) = &result {
        if is_connection_error(e) {
            println!("Skipping test: PostgreSQL appears to be unavailable");
            return;
        }
    }
    assert!(result.is_ok(), "Failed to create sqlx backend");
}

#[tokio::test]
async fn test_sqlx_simple_query() {
    // Create a backend
    let backend = match test_backend().await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create backend: {:?}", e);
        }
    };

    // Create a temporary database using the boxed API
    let ctx = match with_boxed_database(backend)
        .setup(|conn| {
            boxed_async!(async move {
                // Create a table
                let _create_table =
                    sqlx::query("CREATE TABLE test_table (id SERIAL PRIMARY KEY, name TEXT)")
                        .execute(conn.pool_connection())
                        .await?;

                // Insert data
                let _insert_data = sqlx::query("INSERT INTO test_table (name) VALUES ($1)")
                    .bind("test_name")
                    .execute(conn.pool_connection())
                    .await?;

                Ok(())
            })
        })
        .execute()
        .await
    {
        Ok(ctx) => {
            println!("Created test database: {}", ctx.db.name());
            ctx
        }
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create database: {:?}", e);
        }
    };

    // Get a connection
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    // Query data back
    let rows = sqlx::query("SELECT * FROM test_table")
        .fetch_all(conn.pool_connection())
        .await
        .expect("Failed to query data");

    // Verify data
    assert_eq!(rows.len(), 1, "Expected 1 row");
    let name: &str = rows[0].get("name");
    assert_eq!(name, "test_name", "Expected name to be test_name");

    // Return the connection to the pool
    ctx.db
        .pool
        .release(conn)
        .await
        .expect("Failed to release connection");
}

#[tokio::test]
async fn test_sqlx_transaction() {
    // Create a backend
    let backend = match test_backend().await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create backend: {:?}", e);
        }
    };

    // Create a temporary database using the boxed API
    let mut ctx = match with_boxed_database(backend)
        .setup(|conn| {
            boxed_async!(async move {
                // Create a table
                let _create_table =
                    sqlx::query("CREATE TABLE test_tx_table (id SERIAL PRIMARY KEY, name TEXT)")
                        .execute(conn.pool_connection())
                        .await?;
                Ok(())
            })
        })
        .execute()
        .await
    {
        Ok(ctx) => {
            println!(
                "Created test database for transaction test: {}",
                ctx.db.name()
            );
            ctx
        }
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create database: {:?}", e);
        }
    };

    // Get a connection
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    // Create a transaction
    let mut tx = <testkit_core::TestDatabaseInstance<PostgresBackend> as TransactionManager>::begin_transaction(&mut ctx.db)
        .await
        .expect("Failed to begin transaction");

    // Insert data in transaction
    let insert_data = sqlx::query("INSERT INTO test_tx_table (name) VALUES ($1)")
        .bind("tx_test_name")
        .execute(conn.pool_connection())
        .await;
    assert!(insert_data.is_ok(), "Failed to insert data in transaction");

    // Commit the transaction
    <testkit_core::TestDatabaseInstance<PostgresBackend> as TransactionManager>::commit_transaction(&mut tx)
        .await
        .expect("Failed to commit transaction");

    // Verify data
    let rows = sqlx::query("SELECT * FROM test_tx_table")
        .fetch_all(conn.pool_connection())
        .await
        .expect("Failed to query data");

    // Verify data
    assert_eq!(rows.len(), 1, "Expected 1 row");
    let name: &str = rows[0].get("name");
    assert_eq!(name, "tx_test_name", "Expected name to be tx_test_name");

    // Return the connection to the pool when done
    ctx.db
        .pool
        .release(conn)
        .await
        .expect("Failed to release connection");
}

#[tokio::test]
async fn test_sqlx_with_connection() {
    // Create a backend
    let backend = match test_backend().await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create backend: {:?}", e);
        }
    };
    let datbase_config = test_config();
    let backend_clone = backend.clone();
    let _test_context = TestDatabaseInstance::new(backend, datbase_config).await;

    println!("----- created database -----");
    // Create a temporary database using the boxed API
    let backend_clone_2 = backend_clone.clone();

    // Get a connection
    let admin_url_connection_string =
        "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";
    let conn = backend_clone_2
        .connect_with_string(admin_url_connection_string)
        .await
        .unwrap();
    let conn2 = conn.clone();
    let _rows = SqlxConnection::with_connection(admin_url_connection_string, |conn| {
        boxed_async!(async move {
            sqlx::query("DELETE FROM test_table")
                .execute(conn.pool_connection())
                .await?;
            let insert_query = format!("INSERT INTO test_table (name) VALUES ($1)");
            let rows = sqlx::query(&insert_query)
                .bind("boxed_test_name")
                .execute(conn.pool_connection())
                .await?;
            Ok::<_, PostgresError>(rows)
        })
    })
    .await
    .unwrap();
    let rows = sqlx::query("SELECT * FROM test_table")
        .fetch_all(conn2.pool_connection())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let name: &str = rows[0].get("name");
    assert_eq!(
        name, "boxed_test_name",
        "Expected name to be boxed_test_name"
    );
}
#[tokio::test]
async fn test_sqlx_boxed_api() {
    // Create a backend
    let backend = match test_backend().await {
        Ok(backend) => backend,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create backend: {:?}", e);
        }
    };

    // Use the boxed API
    let table_name = "boxed_api_test_table";

    // Use the boxed API to create a database
    let ctx = match with_boxed_database(backend)
        .setup(|conn| {
            let table_name = table_name.to_string();
            boxed_async!(async move {
                let query = format!(
                    "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
                    table_name
                );
                sqlx::query(&query).execute(conn.pool_connection()).await?;

                // Insert a row
                let insert_query = format!("INSERT INTO {} (name) VALUES ($1)", table_name);
                sqlx::query(&insert_query)
                    .bind("boxed_test_name")
                    .execute(conn.pool_connection())
                    .await?;

                Ok(())
            })
        })
        .execute()
        .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            if is_connection_error(&e) {
                println!("Skipping test: PostgreSQL appears to be unavailable");
                return;
            }
            panic!("Failed to create database with boxed API: {:?}", e);
        }
    };

    // Get a connection to query the data
    let conn = ctx
        .db
        .pool
        .acquire()
        .await
        .expect("Failed to acquire connection");

    // Query data
    let query = format!("SELECT * FROM {}", table_name);
    let rows = sqlx::query(&query)
        .fetch_all(conn.pool_connection())
        .await
        .expect("Failed to query data");

    // Verify data
    assert_eq!(rows.len(), 1, "Expected 1 row");
    let name: &str = rows[0].get("name");
    assert_eq!(
        name, "boxed_test_name",
        "Expected name to be boxed_test_name"
    );

    // Return the connection to the pool
    ctx.db
        .pool
        .release(conn)
        .await
        .expect("Failed to release connection");
}
