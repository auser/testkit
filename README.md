# Testkit

A Rust library for managing test databases with support for PostgreSQL, MySQL, and SQLite. It provides an easy way to create isolated database instances for testing, with automatic cleanup and connection pooling.

## Features

- Support for multiple database backends:
  - PostgreSQL (native and SQLx)
  - MySQL
  - SQLite (via SQLx)
- Automatic database cleanup
- Connection pooling
- Database templating for fast test setup
- Async/await support
- Transaction management
- Migration support
- **Built-in logging with tracing** - all operations are logged for easy debugging
- **No need to specify return types** - the library handles type inference for you
- **Automatic user creation and privilege management** - the library creates test users with appropriate permissions

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
db-testkit = { version = "0.2.0", features = ["postgres"] }  # or other backends
```

Available features:
- `postgres` - Native PostgreSQL support
- `mysql` - MySQL support
- `sqlx-postgres` - SQLx PostgreSQL support
- `sqlx-sqlite` - SQLite support

## Usage

### Function-based API (Recommended)

The function-based API provides a clean and simple way to work with test databases:

```rust
use db_testkit::with_test_db;

#[tokio::test]
async fn test_with_postgres() {
    // Initialize tracing for logs (optional but recommended)
    tracing_subscriber::fmt::init();
    
    with_test_db(|db| async move {
        // Setup database with admin permissions
        db.setup(|mut conn| async move {
            conn.execute(
                "CREATE TABLE users (
                    id SERIAL PRIMARY KEY,
                    email TEXT NOT NULL,
                    name TEXT NOT NULL
                )"
            ).await?;
            // No need to specify return types - they're inferred automatically
            Ok(())
        }).await?;

        // Execute tests with regular permissions
        db.test(|mut conn| async move {
            let rows = conn.execute("SELECT * FROM users").await?;
            assert_eq!(rows.len(), 0);
            // No need to specify return types - they're inferred automatically
            Ok(())
        }).await?;

        // No need to specify return types - they're inferred automatically
        Ok(())
    })
    .await;
}
```

### Macro-based API

The library also supports a macro-based API:

```rust
use db_testkit::with_test_db;

#[tokio::test]
async fn test_with_postgres() {
    with_test_db!(|db| async move {
        let test_user = db.test_user.clone();
        
        // Setup database
        db.setup(|mut conn| async move {
            conn.execute(
                "CREATE TABLE users (
                    id SERIAL PRIMARY KEY,
                    email TEXT NOT NULL,
                    name TEXT NOT NULL
                )"
            ).await?;
            Ok(())
        }).await?;

        // Execute tests
        db.test(|mut conn| async move {
            let rows = conn.execute("SELECT * FROM users").await?;
            assert_eq!(rows.len(), 0);
            Ok(())
        }).await?;

        Ok(())
    })
    .await;
}
```

### SQLite Example

```rust
use db_testkit::with_sqlite_test_db;

#[tokio::test]
async fn test_with_sqlite() {
    with_sqlite_test_db(|db| async move {
        let test_user = db.test_user.clone();
        
        // Setup database
        db.setup(|mut conn| async move {
            conn.execute(
                "CREATE TABLE users (
                    id INTEGER PRIMARY KEY,
                    email TEXT NOT NULL,
                    name TEXT NOT NULL
                )"
            ).await?;
            
            // Insert test data
            conn.execute(
                "INSERT INTO users (email, name) VALUES ('" + &test_user + "', 'Test User')"
            ).await?;
            
            Ok(())
        })
        .await?;

        Ok(())
    })
    .await;
}
```

### Environment Setup

Create a `.env` file in your project root:

```env
# For PostgreSQL
DATABASE_URL=postgres://user:password@localhost:5432/postgres

# For MySQL
DATABASE_URL=mysql://user:password@localhost:3306/mysql

# For SQLite
DATABASE_URL=/path/to/sqlite/databases
```

## Logging

The library uses the `tracing` crate for logging. Logging is enabled by default, with no feature flag required. To see logs in your tests, initialize the tracing subscriber at the beginning of your test:

```rust
#[tokio::test]
async fn my_test() {
    // Set the log level if not already set
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "db_testkit=debug,info");
    }
    
    // Initialize the subscriber
    let _ = tracing_subscriber::fmt::try_init();
    
    // Your test code...
}
```

You can customize the log level by setting the `RUST_LOG` environment variable:

```bash
RUST_LOG=db_testkit=debug,postgres=info cargo test
```

The library logs important events such as:
- Database creation and dropping
- Connection acquisition and release
- SQL statement execution
- Error conditions and cleanup operations

This makes debugging database tests much easier.

## Using with_test_db in External Crates

When using the `with_test_db!` macro in an external crate, you need to properly import both the macro and the required types:

```rust
use db_testkit::prelude::*;
use db_testkit::{TestDatabase, with_test_db};
use db_testkit::backends::sqlx::SqlxPostgresBackend;

#[tokio::test]
async fn test_external_crate_usage() {
    // Method 1: Using the macro with type annotation
    with_test_db!(|db: TestDatabase<SqlxPostgresBackend>| async move {
        // Get a connection from the pool
        let mut conn = db.test_pool.acquire().await.unwrap();
        
        // Your test code here
        
        Ok(())
    });
    
    // Method 2: Using the macro with custom URL
    with_test_db!(
        "postgres://postgres:postgres@localhost:5432/postgres?sslmode=disable",
        |db: TestDatabase<SqlxPostgresBackend>| async move {
            // Get a connection from the pool
            let mut conn = db.test_pool.acquire().await.unwrap();
            
            // Your test code here
            
            Ok(())
        }
    );
}

### Common Issues

If you encounter errors like:
- `expected expression, found $`
- `cannot find value backend in this scope`

Make sure you:
1. Import the macro with `use db_testkit::with_test_db;`
2. Import the correct backend type (e.g., `use db_testkit::backends::sqlx::SqlxPostgresBackend;`)
3. Specify the full type in the macro: `|db: TestDatabase<SqlxPostgresBackend>|`

## Contributing

Contributions are welcome! Here's how you can help:

1. Fork the repository
2. Create a new branch: `git checkout -b feature-name`
3. Make your changes
4. Add tests if applicable
5. Run the test suite: `cargo test --all-features`
6. Commit your changes: `git commit -m 'Add feature'`
7. Push to the branch: `git push origin feature-name`
8. Submit a Pull Request

### Development Setup

1. Install Rust and Cargo
2. Install database servers for testing:
   - PostgreSQL
   - MySQL
   - SQLite
3. Copy `.env.example` to `.env` and configure your database URLs
4. Copy `.envrc.example` to `.envrc` and configure your environment variables in development
5. Run tests: `cargo test --all-features`

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions. 