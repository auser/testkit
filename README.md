# Testkit

A Rust library for managing test databases with support for PostgreSQL, MySQL, and SQLite. It provides an easy way to create isolated database instances for testing, with automatic cleanup and connection pooling.

## Usage

```rust
with_database(backend, config, |db| { ... })
    .then(|_| with_transaction(|ctx, tx| { ... create table ... }))
    .then(|_| with_transaction(|ctx, tx| { ... insert user ... }))
    .then(|_| with_transaction(|ctx, tx| { ... get user ... }))
```
