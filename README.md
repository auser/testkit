# Testkit

A Rust library for managing test databases with support for PostgreSQL, MySQL, and SQLite. It provides an easy way to create isolated database instances for testing, with automatic cleanup and connection pooling.

## Usage

```rust
with_database(backend)
    .await
    .setup(|conn| async move { /* setup code */ })
    .with_transaction(|tx| async move { /* transaction code */ })
    .execute()
    .await
```

## Error Handling

All functions return `Result<TestContext<DB>, Error>` or `Result<T, Error>` where appropriate, allowing for proper error handling in your tests.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
