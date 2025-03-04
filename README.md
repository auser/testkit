# Testkit

A Rust library for managing test databases with support for PostgreSQL, MySQL, and SQLite. It provides an easy way to create isolated database instances for testing, with automatic cleanup and connection pooling.

## Features

- Create isolated database instances for each test
- Automatic database cleanup after tests
- Transaction support for test setup and execution
- Backend implementations for PostgreSQL, MySQL (in progress)

## Usage

The library provides two API styles for working with test databases:

### Standard API

The standard API requires manual boxing of closures when they capture local variables:

```rust
use testkit_core::{with_database, DatabaseConfig, boxed_future};
use testkit_postgres::postgres_backend_with_config;

#[tokio::test]
async fn test_database_operations() {
    // Create a PostgreSQL backend
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@localhost:5432/postgres",
        "postgres://postgres:postgres@localhost:5432/postgres",
    );
    let backend = postgres_backend_with_config(config).await.unwrap();
    
    // Local variable we want to use in our setup
    let table_name = "test_table".to_string();
    
    // Use the standard API with manual boxing for captured variables
    let ctx = with_database(backend)
        .setup(boxed_future(move |conn| {
            let table = table_name.clone();
            async move {
                // Create a test table
                conn.client()
                    .execute(
                        &format!("CREATE TABLE {} (id SERIAL PRIMARY KEY, value TEXT NOT NULL)", table),
                        &[],
                    )
                    .await?;
                Ok(())
            }
        }))
        .execute()
        .await
        .unwrap();
        
    // ... Test code using ctx.db ...
}
```

### Automatic Future Boxing API

The boxed API handles local variable capturing automatically by boxing closures for you:

```rust
use testkit_core::{with_boxed_database, DatabaseConfig};
use testkit_postgres::postgres_backend_with_config;

#[tokio::test]
async fn test_database_operations() {
    // Create a PostgreSQL backend
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@localhost:5432/postgres",
        "postgres://postgres:postgres@localhost:5432/postgres",
    );
    let backend = postgres_backend_with_config(config).await.unwrap();
    
    // Local variable we want to use in our setup
    let table_name = "test_table".to_string();
    
    // Use the boxed API which automatically handles closures with captured variables
    let ctx = with_boxed_database(backend)
        .setup(|conn| async move {
            // Create a test table - directly using table_name without cloning
            conn.client()
                .execute(
                    &format!("CREATE TABLE {} (id SERIAL PRIMARY KEY, value TEXT NOT NULL)", table_name),
                    &[],
                )
                .await?;
            Ok(())
        })
        .execute()
        .await
        .unwrap();
        
    // ... Test code using ctx.db ...
}
```

## Database Transaction Support

The library supports both setup and transaction operations:

```rust
let ctx = with_boxed_database(backend)
    .setup(|conn| async move {
        // Setup code - creates tables, initial data, etc.
        Ok(())
    })
    .with_transaction(|conn| async move {
        // Transaction code - inserts test data
        // Will be automatically rolled back after the test
        Ok(())
    })
    .execute()
    .await
    .unwrap();
```

## Choosing Between APIs

- Use the **boxed API** (`with_boxed_database`) when:
  - You need to capture local variables in your closures
  - You want simpler code without manual boxing
  - You don't need control over the lifetime of the closure

- Use the **standard API** (`with_database`) when:
  - You need precise control over closure lifetimes
  - You're working with code that already uses this pattern
  - You prefer explicit boxing control

## Backend Implementations

- **PostgreSQL**: `testkit-postgres` - Complete implementation
- **MySQL**: `testkit-mysql` - In progress

## Example: Complete Test

```rust
use testkit_core::{with_boxed_database, DatabaseConfig};
use testkit_postgres::postgres_backend_with_config;

#[tokio::test]
async fn test_user_creation() {
    // Create backend
    let config = DatabaseConfig::default();
    let backend = postgres_backend_with_config(config).await.unwrap();
    
    // User data for our test
    let username = "test_user".to_string();
    let email = "test@example.com".to_string();
    
    // Create and set up database
    let ctx = with_boxed_database(backend)
        .setup(|conn| async move {
            // Create users table
            conn.client()
                .execute(
                    "CREATE TABLE users (
                        id SERIAL PRIMARY KEY,
                        username TEXT NOT NULL UNIQUE,
                        email TEXT NOT NULL UNIQUE
                    )",
                    &[],
                )
                .await?;
            Ok(())
        })
        .with_transaction(|conn| async move {
            // Insert test user
            conn.client()
                .execute(
                    "INSERT INTO users (username, email) VALUES ($1, $2)",
                    &[&username, &email],
                )
                .await?;
            Ok(())
        })
        .execute()
        .await
        .unwrap();
    
    // Test code - verify user was created
    let conn = ctx.db.pool.acquire().await.unwrap();
    let rows = conn.client()
        .query("SELECT * FROM users WHERE username = $1", &[&username])
        .await
        .unwrap();
    
    assert_eq!(rows.len(), 1, "User should exist");
    let db_email: String = rows[0].get("email");
    assert_eq!(db_email, email, "Email should match");
}
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
