# Database Testing Framework Design Pattern

## Overview
We're developing a Rust database testing framework that uses a functional, composable approach with zero-cost abstractions. The pattern enables flexible transaction handling and setup operations using ergonomic APIs.

## Key Components

1. **Core Traits**:
   - `TransactionHandler`: Central trait for operations with database context
   - `IntoTransactionHandler`: Conversion trait for creating handlers
   - `TransactionHandlerExt`: Extension trait adding combinators to handlers

2. **Handler Types**:
   - `SetupHandler`: For database initialization
   - `TransactionFnHandler`: For executing functions in a transaction
   - `DatabaseTransactionHandler`: Combining transactions with database context
   - `AndThenHandler`: Chaining multiple handlers

3. **Entry Points**:
   - Fluent API: `with_database().setup(...).with_transaction(...).execute()`
   - Functional API: `setup(...).and_then(|_| with_transaction(...))` with `run_with_database()`

## Usage Patterns

### Fluent API
```rust
let ctx = with_database(backend)
    .await
    .setup(|conn| async move {
        // Setup database schema
        Ok(())
    })
    .with_transaction(|tx| async move {
        // Perform operations in a transaction
        Ok(())
    })
    .execute()
    .await?;
```

### Functional API
```rust
let handler = setup(|conn| async move {
    // Setup database schema
    Ok(())
}).and_then(|_| {
    with_transaction(|tx| async move {
        // Perform operations in a transaction
        Ok(())
    })
});

let ctx = run_with_database(backend, handler).await?;
```

## Benefits
- Composable operations through `and_then` and other combinators
- Separation of concerns with modular handlers
- Zero-cost abstractions using compile-time generics
- Type-safe API with good ergonomics
- Optional setup phase
- Flexible error handling

This pattern follows functional programming principles while leveraging Rust's strong type system to provide a safe and efficient API for database testing.