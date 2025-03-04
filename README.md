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
