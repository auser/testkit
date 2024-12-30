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

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
db-testkit = { version = "0.1.0", features = ["postgres"] }  # or other backends
```

Available features:
- `postgres` - Native PostgreSQL support
- `mysql` - MySQL support
- `sqlx-postgres` - SQLx PostgreSQL support
- `sqlx-sqlite` - SQLite support

## Usage

### PostgreSQL Example

```rust
use db_testkit::with_test_db;

#[tokio::test]
async fn test_with_postgres() {
    with_test_db(|db| async move {
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
            
            // Insert test data
            conn.execute(
                "INSERT INTO users (email, name) VALUES ($1, $2)",
                &[&test_user, "Test User"],
            ).await?;
            
            Ok(())
        })
        .await?;

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
                "INSERT INTO users (email, name) VALUES (?, ?)",
                &[&test_user, "Test User"],
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