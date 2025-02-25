# Purpose of db-testkit

This crate is a testkit for testing database code against multiple database engines in an elegant way.

It should be an abstraction layer of database engines, so that the database code can be tested against different database engines without changing the code. For this to be successful, it should be able to support the following:

- Multiple relational database engines (PostgreSQL, SQLite, MySQL, etc.)
- Creation of new databases for each engine
- Creation of new tables and columns within the database
- Running migrations or cloning a production database
- Executing raw SQL queries
- Executing transactions within the database
- Dropping and deleting a database once a single test is complete.

## Usage

```rust
#[tokio::test]
async fn test_get_organization() {
  with_test_db!(|db| async move {
    db.setup(|mut conn| async move {
      // Arbitrary code to setup the database with
      // a user with no restraints (superuser)
      Ok(())
    }).await;

    // Now we can use the database pool to test the code
    let org = get_organization(&mut conn).await;
    // Or we can get a transaction from the pool
    let mut tx = db.begin().await;
    // Run arbitrary code within the transaction
    let org = get_organization(&mut tx).await;
    assert!(org.is_ok());
    // Commit the transaction
    tx.commit().await;
  }).await;
  // After this test is complete, the database will be dropped
  // and the connection will be closed.
  // Using the impl Drop trait, the database will be dropped
  // even if the test panics.
}
```

## General design

The main idea is to provide a trait `DatabasePool` that can be implemented for different database engines and used in an elegant way.

