use testkit_core::{
    with_database, with_transaction, DatabaseConfig, TestDatabaseInstance, Transaction,
    TransactionManager,
};
use testkit_postgres::{postgres, Error as PgError, PostgresBackend};

// A simple user struct for our example
#[derive(Debug, Clone)]
struct User {
    id: i32,
    name: String,
}

fn main() {
    println!("PostgreSQL Example - Database Testing Pattern");
    println!("============================================");

    // This example demonstrates the intended usage pattern
    // without actually executing database operations

    println!("\nThe intended pattern is:");
    println!("1. Create a TestDatabaseInstance");
    println!("2. Define operations with with_database and with_transaction");
    println!("3. Execute operations on the database instance");

    println!("\nExample code (not actually executed):");
    println!("```rust");
    println!("// Create test database instance");
    println!("let mut db_instance = TestDatabaseInstance::new(postgres(), DatabaseConfig::default()).await?;");

    println!("\n// Define database operations");
    println!("let operation = with_database(postgres(), DatabaseConfig::default(), |db| {{");
    println!("    // We can execute operations on the database");
    println!("    // Define a transaction to create users table");
    println!("    let create_users = with_transaction(|_ctx, tx| {{");
    println!("        async move {{");
    println!("            // Execute SQL query to create table");
    println!("            Ok(())");
    println!("        }}");
    println!("    }});");

    println!("    // Define a transaction to insert a user");
    println!("    let insert_user = with_transaction(|_ctx, tx| {{");
    println!("        async move {{");
    println!("            // Execute SQL query to insert user");
    println!("            let user = User {{ id: 1, name: \"Alice\".to_string() }};");
    println!("            Ok(user)");
    println!("        }}");
    println!("    }});");

    println!("    // Execute operations in sequence");
    println!("    Box::pin(async move {{");
    println!("        create_users.execute(db).await?;");
    println!("        let user = insert_user.execute(db).await?;");
    println!("        Ok(user)");
    println!("    }})");
    println!("}});");

    println!("\n// Execute the database operation on our instance");
    println!("let result = operation.execute(&mut db_instance).await?;");
    println!("```");

    println!("\nThis pattern allows you to:");
    println!("1. Define reusable database operations");
    println!("2. Compose operations together");
    println!("3. Execute them on a test database instance");
    println!("4. Clean up automatically when the instance is dropped");
}
