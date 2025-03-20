# Testkit

A Rust library for managing test databases with support for PostgreSQL, MySQL, and SQLite. It provides an easy way to create isolated database instances for testing, with automatic cleanup and connection pooling.

## Features

- Create isolated database instances for each test
- Automatic database cleanup after tests
- Transaction support for test setup and execution
- Backend implementations for PostgreSQL, MySQL (in progress)
- **Fluent API** for intuitive and readable test setup

## Configuration Options

### Feature Flags

The following feature flags are available:

For postgres:

```bash
cargo add testkit-postgres
```

- **Default Features:**
  - **`with-tokio-postgres`** - Enables PostgreSQL support via tokio-postgres (enabled by default)

- **Optional Features:**
  - **`with-sqlx`** - Enables PostgreSQL support via SQLx

For mysql:

```bash
cargo add testkit-mysql
```

- **Default Features:**
  - **`with-mysql-native-tls`** - Enables MySQL support via mysql-native-tls (enabled by default)

- **Optional Features:**
  - **`with-sqlx`** - Enables MySQL support via SQLx
  
**Note:** The `with-tokio-postgres` and `with-sqlx` features are mutually exclusive. Only enable one of these features at a time.

## Command Line Interface (CLI)

TestKit includes a command-line tool for managing test databases. First, install the required packages:

```bash
# Install the CLI tool
cargo add testkit-cli
```

After installation, you can use the CLI:

```bash
# List all test databases with prefix "testkit"
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres"
testkit list -d postgres --prefix testkit

# List all test MySQL databases
export DATABASE_URL="mysql://root:root@localhost:3306/mysql"
testkit list -d mysql --prefix testkit

# Reset (drop) all test databases with prefix "testkit"
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres"
testkit reset -d postgres --prefix testkit
```

The CLI supports both PostgreSQL and MySQL databases and provides useful commands for:

- **Listing** test databases to see which ones exist
- **Resetting** (dropping) test databases to clean up after test runs
- **Debugging** connection issues with detailed output

This is particularly useful for:
- Cleaning up orphaned test databases after failed test runs
- Managing test databases in CI/CD environments
- Troubleshooting database connection issues

### Environment Variables

The library uses the following environment variables for configuration:

- **`DATABASE_URL`** - Connection URL for regular database operations
- **`ADMIN_DATABASE_URL`** - Connection URL for admin operations (create/drop databases)

## Configuration

### Using `DatabaseConfig`

The `DatabaseConfig` struct is used to configure connections:

```rust
// Create a new configuration with explicit connection strings
let config = DatabaseConfig::new(
    "postgres://postgres:postgres@localhost:5432/postgres", // admin URL
    "postgres://testuser:password@localhost:5432/postgres"  // user URL
);

// Or use the default configuration from environment variables
let config = DatabaseConfig::default(); // reads from ADMIN_DATABASE_URL and DATABASE_URL
```

### Connection Pooling

The library manages database connection pools for you, with configurable connection limits:

```rust
// Configure with custom connection pool size
let mut config = DatabaseConfig::default();
config.max_connections = Some(5); // Limit to 5 connections in the pool
```

## Backend-Specific Features

### PostgreSQL

The PostgreSQL backend supports two driver options:

#### With tokio-postgres

```rust
// Enable the default tokio-postgres implementation
use testkit_postgres::postgres_backend_with_config;

let backend = postgres_backend_with_config(config).await.unwrap();
```

#### With SQLx

```rust
// Enable the SQLx implementation
use testkit_postgres::sqlx_backend_with_config;

let backend = sqlx_backend_with_config(config).await.unwrap();
```

To use SQLx instead of tokio-postgres, update your `Cargo.toml`:

```toml
[dependencies]
testkit-postgres = { version = "0.1.0", default-features = false, features = ["with-sqlx"] }
```

## Complete Example for Real-World Testing

```rust
use testkit_core::{with_boxed_database, DatabaseConfig};
use testkit_postgres::postgres_backend_with_config;

#[tokio::test]
async fn test_user_registration() {
    // Setup configuration
    let config = DatabaseConfig::new(
        std::env::var("ADMIN_DATABASE_URL").unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string()),
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://testuser:password@localhost:5432/postgres".to_string()),
    );
    
    // Create the backend
    let backend = postgres_backend_with_config(config).await.unwrap();
    
    // Initialize a test database with schema and test data
    let ctx = with_boxed_database(backend)
        .setup(|conn| async move {
            // Create tables and initial schema
            conn.client()
                .execute(
                    "CREATE TABLE users (
                        id SERIAL PRIMARY KEY,
                        email TEXT UNIQUE NOT NULL,
                        password_hash TEXT NOT NULL,
                        created_at TIMESTAMPTZ DEFAULT NOW()
                    )",
                    &[],
                )
                .await?;
                
            Ok(())
        })
        .with_transaction(|conn| async move {
            // Add test data that will be rolled back after test
            conn.client()
                .execute(
                    "INSERT INTO users (email, password_hash) VALUES ($1, $2)",
                    &[&"test@example.com", &"hashed_password"],
                )
                .await?;
                
            Ok(())
        })
        .execute()
        .await
        .unwrap();
    
    // Run your actual test against the db.pool
    let conn = ctx.db.pool.acquire().await.unwrap();
    
    // Example: Call your application code that uses the database
    // let user_service = UserService::new(conn.clone());
    // let result = user_service.register("newuser@example.com", "password123").await;
    
    // Make assertions about the result
    // assert!(result.is_ok());
}
```

## Database Creation Process

The `setup()` function uses the admin connection (specified via `ADMIN_DATABASE_URL`) to create new databases without requiring the test user to have database creation permissions. This allows tests to run with minimal privileges while still being able to create isolated test databases.

When a test is initialized:

1. A unique database name is generated
2. The admin connection is used to create the database
3. The regular user connection is used for all subsequent operations

This separation ensures your tests run with appropriate permissions while still maintaining isolation.

## Usage

The library provides multiple API styles for working with test databases, with the fluent API being the most user-friendly option.

### Fluent API (Recommended)

The fluent API offers a clean, readable way to set up and use test databases:

```rust
use testkit_core::{with_boxed_database, DatabaseConfig};
use testkit_postgres::postgres_backend_with_config;

#[tokio::test]
async fn test_with_fluent_api() {
    // Create the backend
    let config = DatabaseConfig::default();
    let backend = postgres_backend_with_config(config).await.unwrap();
    
    // Use the fluent API for intuitive and readable test setup
    let ctx = with_boxed_database(backend)
        // Setup the database schema
        .setup(|conn| async move {
            conn.client()
                .execute(
                    "CREATE TABLE products (id SERIAL PRIMARY KEY, name TEXT, price DECIMAL)",
                    &[],
                )
                .await?;
            Ok(())
        })
        // Add test data in a transaction (will be rolled back after the test)
        .with_transaction(|conn| async move {
            conn.client()
                .execute(
                    "INSERT INTO products (name, price) VALUES ($1, $2)",
                    &[&"Test Product", &19.99],
                )
                .await?;
            Ok(())
        })
        // Execute the test setup and get the context
        .execute()
        .await
        .unwrap();
    
    // Use the database in your test
    let conn = ctx.db.pool.acquire().await.unwrap();
    // ... test code ...
}
```

#### Benefits of the Fluent API

- **Readability**: Clear chain of operations that reads like English
- **Type Safety**: Full type checking at compile time
- **Composability**: Easy to add or remove steps in your test setup
- **Error Handling**: Consistent error propagation through the chain
- **Automatic Resource Management**: Connections and transactions are managed for you

### Standard API

The standard API requires manual boxing of closures when they capture local variables:

```rust
use testkit_core::{with_database, DatabaseConfig, boxed_future};
use testkit_postgres::postgres_backend_with_config;

#[tokio::test]
async fn test_database_operations() {
    // Create a PostgreSQL backend
    let config = DatabaseConfig::new(
        "postgres://postgres:postgres@localhost:5432/postgres",
        "postgres://postgres:postgres@localhost:5432/postgres",
    );
    let backend = postgres_backend_with_config(config).await.unwrap();
    
    // Local variable we want to use in our setup
    let table_name = "test_table".to_string();
    
    // Use the standard API with manual boxing for captured variables
    let ctx = with_database(backend)
        .setup(boxed_future(move |conn| {
            let table = table_name.clone();
            async move {
                // Create a test table
                conn.client()
                    .execute(
                        &format!("CREATE TABLE {} (id SERIAL PRIMARY KEY, value TEXT NOT NULL)", table),
                        &[],
                    )
                    .await?;
                Ok(())
            }
        }))
        .execute()
        .await
        .unwrap();
        
    // ... Test code using ctx.db ...
}
```

### Automatic Future Boxing API

The boxed API handles local variable capturing automatically by boxing closures for you:

```rust
use testkit_core::{with_boxed_database, DatabaseConfig};
use testkit_postgres::postgres_backend_with_config;

#[tokio::test]
async fn test_database_operations() {
    // Create a PostgreSQL backend
    let config = DatabaseConfig::new(
        // Admin connection
        "postgres://postgres:postgres@localhost:5432/postgres",
        // User connection
        "postgres://postgres:postgres@localhost:5432/app_database",
    );
    let backend = postgres_backend_with_config(config).await.unwrap();
    
    // Local variable we want to use in our setup
    let table_name = "test_table".to_string();
    
    // Use the boxed API which automatically handles closures with captured variables
    let ctx = with_boxed_database(backend)
        .setup(|conn| async move {
            // Create a test table - directly using table_name without cloning
            conn.client()
                .execute(
                    &format!("CREATE TABLE {} (id SERIAL PRIMARY KEY, value TEXT NOT NULL)", table_name),
                    &[],
                )
                .await?;
            Ok(())
        })
        .execute()
        .await
        .unwrap();
        
    // ... Test code using ctx.db ...
}
```

## Database Transaction Support

The library supports both setup and transaction operations:

```rust
let ctx = with_boxed_database(backend)
    .setup(|conn| async move {
        // Setup code - creates tables, initial data, etc.
        Ok(())
    })
    .with_transaction(|conn| async move {
        // Transaction code - inserts test data
        // Will be automatically rolled back after the test
        Ok(())
    })
    .execute()
    .await
    .unwrap();
```

## Choosing Between APIs

- Use the **boxed API** (`with_boxed_database`) when:
  - You need to capture local variables in your closures
  - You want simpler code without manual boxing
  - You don't need control over the lifetime of the closure

- Use the **standard API** (`with_database`) when:
  - You need precise control over closure lifetimes
  - You're working with code that already uses this pattern
  - You prefer explicit boxing control

## Backend Implementations

- **PostgreSQL**: `testkit-postgres` - Complete implementation
- **MySQL**: `testkit-mysql` - In progress

## Example: Complete Test

```rust
use testkit_core::{with_boxed_database, DatabaseConfig};
use testkit_postgres::postgres_backend_with_config;

#[tokio::test]
async fn test_user_creation() {
    // Create backend
    let config = DatabaseConfig::default();
    let backend = postgres_backend_with_config(config).await.unwrap();
    
    // User data for our test
    let username = "test_user".to_string();
    let email = "test@example.com".to_string();
    
    // Create and set up database
    let ctx = with_boxed_database(backend)
        .setup(|conn| async move {
            // Create users table
            conn.client()
                .execute(
                    "CREATE TABLE users (
                        id SERIAL PRIMARY KEY,
                        username TEXT NOT NULL UNIQUE,
                        email TEXT NOT NULL UNIQUE
                    )",
                    &[],
                )
                .await?;
            Ok(())
        })
        .with_transaction(|conn| async move {
            // Insert test user
            conn.client()
                .execute(
                    "INSERT INTO users (username, email) VALUES ($1, $2)",
                    &[&username, &email],
                )
                .await?;
            Ok(())
        })
        .execute()
        .await
        .unwrap();
    
    // Test code - verify user was created
    let conn = ctx.db.pool.acquire().await.unwrap();
    let rows = conn.client()
        .query("SELECT * FROM users WHERE username = $1", &[&username])
        .await
        .unwrap();
    
    assert_eq!(rows.len(), 1, "User should exist");
    let db_email: String = rows[0].get("email");
    assert_eq!(db_email, email, "Email should match");
}
```

## Error Handling

All functions return `Result<TestContext<DB>, Error>` or `Result<T, Error>` where appropriate, allowing for proper error handling in your tests.

### Custom Error Handling Example

```rust
// Handle database connection errors appropriately
let backend = match postgres_backend_with_config(config).await {
    Ok(b) => b,
    Err(e) => {
        tracing::error!("Failed to create database backend: {}", e);
        panic!("Database connection failed: {}", e);
    }
};

// Or using ? for more concise error propagation
async fn setup_test_database() -> Result<TestContext<PostgresBackend>, PostgresError> {
    let config = DatabaseConfig::default();
    let backend = postgres_backend_with_config(config).await?;
    
    // Use the question mark operator to propagate errors
    let ctx = with_boxed_database(backend)
        .setup(|conn| async move {
            // Database setup code
            Ok(())
        })
        .execute()
        .await?;
    
    Ok(ctx)
}
```

## Automatic Cleanup

The library automatically cleans up test databases when the test context is dropped. This ensures that test databases don't persist after tests complete, even if a test fails or panics.

## Admin vs User Connections

The admin connection is used only for database creation and other privileged operations. The user connection is used for all regular database operations during testing.

This separation offers several advantages:

1. **Security**: You can run tests with a database user that has limited permissions
2. **Isolation**: Each test gets its own isolated database instance
3. **Realistic testing**: Tests run with the same permission level as your application

### Best Practices

For maximum security, it's recommended to:

1. Use a dedicated admin user that has CREATE DATABASE permissions
2. Use a regular application user for the user connection
3. Store connection strings securely, especially the admin credentials

## Important Notes

- Features `with-tokio-postgres` and `with-sqlx` are mutually exclusive - only use one at a time
- The admin connection is only used to create/drop databases, minimizing privileged operations
- Each test gets a unique, isolated database with a random name

## Implementing PostgreSQL Support

To support PostgreSQL with both `tokio-postgres` and `sqlx/postgres`, we need to implement the following in the `testkit-postgres` crate:

### Required Implementations for Both Features

1. Custom error types that implement the necessary traits
2. Connection and connection pool abstractions
3. Transaction management
4. Database creation and cleanup logic

### `with-tokio-postgres` Feature (tokio-postgres)

For the `tokio-postgres` implementation, we need to implement:

1. **`PostgresBackend`** - Implementing the `DatabaseBackend` trait using tokio-postgres
2. **`PostgresPool`** - Implementing the `DatabasePool` trait for connection pooling
3. **`PostgresConnection`** - Implementing the `TestDatabaseConnection` trait 
4. **`PostgresTransaction`** - Implementing the `DatabaseTransaction` trait
5. **Transaction Manager** - Implementing the `DBTransactionManager` trait

### `with-sqlx` Feature (sqlx/postgres)

For the `sqlx/postgres` implementation, we need to implement:

1. **`SqlxPostgresBackend`** - Implementing the `DatabaseBackend` trait using sqlx
2. **`SqlxPool`** - Implementing the `DatabasePool` trait for the sqlx pool
3. **`SqlxConnection`** - Implementing the `TestDatabaseConnection` trait
4. **`SqlxTransaction`** - Implementing the `DatabaseTransaction` trait
5. **Transaction Manager** - Implementing the `DBTransactionManager` trait

## Implementing a Custom Backend

You can implement your own database backends by implementing the core traits provided by the library:

### Required Traits

To implement a new database backend, you need to implement these key traits:

1. **`DatabaseBackend`** - The main trait defining a database backend
2. **`DatabasePool`** - Pool management for your database connections
3. **`DatabaseTransaction`** - Transaction handling for your database
4. **`TestDatabaseConnection`** - Connection management for your database

### Example: Implementing a New Backend

Here's a skeleton implementation for a hypothetical database:

```rust
use async_trait::async_trait;
use testkit_core::{
    DatabaseBackend, DatabasePool, DatabaseTransaction, 
    TestDatabaseConnection, DatabaseConfig, DatabaseName
};
use std::sync::Arc;

// 1. Define your error type
#[derive(Debug, Clone)]
pub struct MyDBError(String);

impl std::fmt::Display for MyDBError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MyDB Error: {}", self.0)
    }
}

impl std::error::Error for MyDBError {}

impl From<String> for MyDBError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// 2. Implement connection type
#[derive(Clone)]
pub struct MyDBConnection {
    // Your connection details here
    client: Arc<MyDbClient>, // Replace with your actual client type
}

// 3. Implement connection pool
#[derive(Clone)]
pub struct MyDBPool {
    pool: Arc<MyDbPool>, // Replace with your actual pool type
    connection_string: String,
}

#[async_trait]
impl DatabasePool for MyDBPool {
    type Connection = MyDBConnection;
    type Error = MyDBError;
    
    async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
        // Implement connection acquisition
        let client = self.pool.get()
            .await
            .map_err(|e| MyDBError(e.to_string()))?;
            
        Ok(MyDBConnection { 
            client: Arc::new(client) 
        })
    }
    
    // Implement other required methods...
}

// 4. Implement backend
#[derive(Clone, Debug)]
pub struct MyDBBackend {
    config: DatabaseConfig,
}

#[async_trait]
impl DatabaseBackend for MyDBBackend {
    type Connection = MyDBConnection;
    type Pool = MyDBPool;
    type Error = MyDBError;
    
    async fn new(config: DatabaseConfig) -> Result<Self, Self::Error> {
        // Validate the config
        if config.admin_url.is_empty() || config.user_url.is_empty() {
            return Err(MyDBError("Admin and user URLs must be provided".into()));
        }
        
        Ok(Self { config })
    }
    
    async fn create_pool(
        &self,
        name: &DatabaseName,
        _config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error> {
        // Implement pool creation
        let connection_string = self.connection_string(name);
        // Create your pool using the connection string
        
        // For this example, we'll use a placeholder
        let pool = MyDbPool::new(&connection_string)
            .map_err(|e| MyDBError(e.to_string()))?;
            
        Ok(MyDBPool {
            pool: Arc::new(pool),
            connection_string,
        })
    }
    
    async fn connect_with_string(
        &self,
        connection_string: &str,
    ) -> Result<Self::Connection, Self::Error> {
        // Implement direct connection
        let client = MyDbClient::connect(connection_string)
            .await
            .map_err(|e| MyDBError(e.to_string()))?;
            
        Ok(MyDBConnection {
            client: Arc::new(client),
        })
    }
    
    async fn create_database(
        &self,
        _pool: &Self::Pool,
        name: &DatabaseName,
    ) -> Result<(), Self::Error> {
        // Implement database creation using admin connection
        let admin_client = MyDbClient::connect(&self.config.admin_url)
            .await
            .map_err(|e| MyDBError(e.to_string()))?;
            
        // Create the database
        let db_name = name.as_str();
        admin_client
            .execute(&format!("CREATE DATABASE {}", db_name))
            .await
            .map_err(|e| MyDBError(e.to_string()))?;
            
        Ok(())
    }
    
    fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
        // Implement database cleanup logic
        // This could be async, but for this example we'll use a blocking approach
        let admin_client = MyDbClient::connect_blocking(&self.config.admin_url)
            .map_err(|e| MyDBError(e.to_string()))?;
            
        let db_name = name.as_str();
        admin_client
            .execute(&format!("DROP DATABASE IF EXISTS {}", db_name))
            .map_err(|e| MyDBError(e.to_string()))?;
            
        Ok(())
    }
    
    fn connection_string(&self, name: &DatabaseName) -> String {
        // Construct the connection string for a specific database
        // You'll need to modify this for your specific database URL format
        let db_name = name.as_str();
        let mut url = url::Url::parse(&self.config.user_url)
            .expect("Failed to parse URL");
            
        url.set_path(db_name);
        url.to_string()
    }
}

// 5. Implement any transaction handling if required

// 6. Provide a helper function to create the backend
pub async fn mydb_backend_with_config(config: DatabaseConfig) 
    -> Result<MyDBBackend, MyDBError> 
{
    MyDBBackend::new(config).await
}
```

### Integration With Testkit

After implementing your backend, you can use it with the testkit API:

```rust
use testkit_core::{with_boxed_database, DatabaseConfig};
use mydb_backend::{mydb_backend_with_config};

#[tokio::test]
async fn test_with_custom_backend() {
    let config = DatabaseConfig::default();
    let backend = mydb_backend_with_config(config).await.unwrap();
    
    let ctx = with_boxed_database(backend)
        .setup(|conn| async move {
            // Setup code specific to your database
            Ok(())
        })
        .execute()
        .await
        .unwrap();
        
    // Use the database in your test
}
```

### Key Considerations When Implementing a Backend

1. **Connection Pooling**: Implement efficient connection pooling for your database
2. **Error Handling**: Define clear error types and proper error propagation
3. **Resource Cleanup**: Ensure database instances are properly cleaned up
4. **Transaction Support**: Implement proper transaction handling if your database supports it
5. **Security**: Make proper use of the admin vs. user connection separation

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
