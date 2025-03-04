# Testkit

A Rust library for managing test databases with support for PostgreSQL, MySQL, and SQLite. It provides an easy way to create isolated database instances for testing, with automatic cleanup and connection pooling.

## Usage

Basic API:

```rust
with_database(backend)
    .setup(|conn| async move { /* setup code */ })
    .with_transaction(|tx| async move { /* transaction code */ })
    .execute()
    .await
```

### Automatic Future Boxing API

If you need to capture local variables in your closures and are running into lifetime issues, use the boxed API:

```rust
// Local variable that would cause lifetime issues with standard API
let table_name = "users".to_string();

with_boxed_database(backend)
    .setup(|conn| async move {
        // Can safely capture table_name
        let query = format!("CREATE TABLE {}", table_name);
        conn.execute_query(&query).await?;
        Ok(())
    })
    .with_transaction(|tx| async move {
        // Transaction code
        Ok(())
    })
    .execute()
    .await
```

## Error Handling

All functions return `Result<TestContext<DB>, Error>` or `Result<T, Error>` where appropriate, allowing for proper error handling in your tests.

## Implementing PostgreSQL Support

To support PostgreSQL with both `tokio-postgres` and `sqlx/postgres`, we need to implement the following in the `testkit-postgres` crate:

### Required Implementations for Both Features

1. Custom error types that implement the necessary traits
2. Connection and connection pool abstractions
3. Transaction management
4. Database creation and cleanup logic

### `postgres` Feature (tokio-postgres)

For the `tokio-postgres` implementation, we need to implement:

1. **`PostgresBackend`** - Implementing the `DatabaseBackend` trait using tokio-postgres
2. **`PostgresPool`** - Implementing the `DatabasePool` trait for connection pooling
3. **`PostgresConnection`** - Implementing the `TestDatabaseConnection` trait 
4. **`PostgresTransaction`** - Implementing the `DatabaseTransaction` trait
5. **Transaction Manager** - Implementing the `DBTransactionManager` trait

### `sqlx` Feature (sqlx/postgres)

For the `sqlx/postgres` implementation, we need to implement:

1. **`SqlxPostgresBackend`** - Implementing the `DatabaseBackend` trait using sqlx
2. **`SqlxPool`** - Implementing the `DatabasePool` trait for the sqlx pool
3. **`SqlxConnection`** - Implementing the `TestDatabaseConnection` trait
4. **`SqlxTransaction`** - Implementing the `DatabaseTransaction` trait
5. **Transaction Manager** - Implementing the `DBTransactionManager` trait

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
