# TestKit Core

A flexible and ergonomic database testing framework for Rust applications.

## Key Features

- **Simple API** for common database testing patterns
- **Composable Handlers** for more complex scenarios
- **Async/Await** support for modern Rust applications
- **Multiple Database Backends** support

## Simplified API

The simplified API provides straightforward functions for common database testing patterns:

```rust
// Just initialize a test database
let ctx = test_db(backend).await?;

// Initialize a test database and run setup code
let ctx = test_db_with_setup(backend, |conn| async {
    // Your setup code here
    Ok(())
}).await?;

// Initialize a test database and run a transaction
let ctx = test_db_with_transaction(backend, |conn| async {
    // Your transaction code here
    Ok(())
}).await?;

// Initialize a test database, run setup, then run a transaction
let ctx = test_db_with_setup_and_transaction(
    backend,
    |conn| async { /* setup */ Ok(()) },
    |conn| async { /* transaction */ Ok(()) }
).await?;

// Run a function with a database connection
let result = with_connection(&ctx.db, |conn| async {
    // Your code using the connection
    Ok(42)
}).await?;
```

## Advanced Usage with Handlers

For more complex scenarios, you can use the composable handler API:

```rust
// Create a database entry point
let handler = with_database(backend);

// Setup the database
let handler = handler.setup(|conn| async {
    // Your setup code here
    Ok(())
});

// Add a transaction
let handler = handler.with_transaction(|conn| async {
    // Your transaction code here
    Ok(())
});

// Execute the handler chain
let ctx = handler.execute().await?;
```

### Macro Usage

The `testkit` crate also provides a macro for creating test fixtures:

```rust
```rust
// Create a database entry point
let handler = with_database!(backend)
    .setup!(|conn| async {
        // Your setup code here
        Ok(())
    })
    .with_transaction!(|conn| async {
        // Your transaction code here
        Ok(())
    }).execute().await.unwrap();
```

## Error Handling

All functions return `Result<TestContext<DB>, Error>` or `Result<T, Error>` where appropriate, allowing for proper error handling in your tests.

