// fluent_api.rs - Demonstrates a fluent API pattern with database operations

// Import the testkit-core library
use testkit_core::{with_database, DatabaseConfig, DatabaseName};

// A simple user struct for our example
#[derive(Debug, Clone)]
struct User {
    id: i32,
    name: String,
}

// The main function that demonstrates the fluent API concept
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Fluent API Example");
    println!("=================");

    // In a real application, we would use a real database backend.
    // For this example, we'll show the conceptual flow without actual DB operations

    println!("\nExample of a fluent API for database operations:");
    println!("1. Create a database");
    println!("2. Create a table");
    println!("3. Insert a user");
    println!("4. Retrieve the user");

    // This would be a typical flow using the fluent API:
    // DatabaseBuilder::new()
    //     .create_database("test_db")
    //     .create_table("users")
    //     .insert_user(user)
    //     .get_user(1)
    //     .execute()

    // Create a test database name - this is from the actual testkit-core
    let db_name = DatabaseName::new(None);
    println!("\nGenerated database name: {}", db_name);

    // Create a test database config - this is from the actual testkit-core
    let config = DatabaseConfig::default();
    println!("Using database config: {:?}", config);

    // Create a user object
    let user = User {
        id: 1,
        name: "John Doe".to_string(),
    };

    // Demonstrate the expected flow for transaction chaining
    println!("\nFlow of operations with fluent API:");
    println!("1. Create table: CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))");
    println!("2. Insert user: INSERT INTO users VALUES (1, 'John Doe')");
    println!("3. Get user: SELECT * FROM users WHERE id = 1");
    println!("4. Return result: User {{ id: 1, name: 'John Doe' }}");

    // In the actual implementation, we would use the with_database and with_transaction
    // functions to compose operations in a fluent style:
    //
    // with_database(backend, config, |db| { ... })
    //   .then(|_| with_transaction(|ctx, tx| { ... create table ... }))
    //   .then(|_| with_transaction(|ctx, tx| { ... insert user ... }))
    //   .then(|_| with_transaction(|ctx, tx| { ... get user ... }))

    println!("\nFinal result: {:?}", user);

    Ok(())
}
