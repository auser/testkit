// Simple test to verify exports
#[test]
fn test_module_exports() {
    // Just verify that we can reference these types
    let _: Option<crate::DatabaseConfig> = None;
    let _: Option<crate::DatabaseName> = None;
}

// Test for the DatabaseName type
#[test]
fn test_database_name_creation() {
    use crate::DatabaseName;

    // Create a new database name with a random suffix
    let db_name = DatabaseName::new(None);
    assert!(
        db_name.to_string().starts_with("testkit_"),
        "Database name should start with 'testkit_'"
    );

    // Create a database name with a specific prefix
    let custom_db_name = DatabaseName::new(Some("custom_prefix"));
    assert!(
        custom_db_name.to_string().starts_with("custom_prefix_"),
        "Database name should start with the provided prefix"
    );
}

// Test for the DatabaseConfig type
#[test]
fn test_database_config() {
    use crate::DatabaseConfig;

    // Create a config with explicit URLs
    let admin_url = "postgres://admin@localhost/testdb";
    let user_url = "postgres://user@localhost/testdb";
    let config = DatabaseConfig::new(admin_url.to_string(), user_url.to_string());

    assert_eq!(config.admin_url, admin_url);
    assert_eq!(config.user_url, user_url);
}

// Mock implementation for testing the Transaction trait
#[test]
fn test_transaction_trait() {
    use crate::Transaction;
    use async_trait::async_trait;

    // Simple context for testing
    #[derive(Debug, Clone)]
    struct TestContext {
        value: i32,
    }

    // Simple transaction for testing
    struct TestTransaction {
        modifier: i32,
    }

    #[async_trait]
    impl Transaction for TestTransaction {
        type Context = TestContext;
        type Item = i32;
        type Error = String;

        async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
            ctx.value += self.modifier;
            Ok(ctx.value)
        }
    }

    // Test executing a transaction
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let tx = TestTransaction { modifier: 5 };
            let mut ctx = TestContext { value: 10 };

            let result = tx.execute(&mut ctx).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 15);
            assert_eq!(ctx.value, 15);
        });
}

// Test for the result operator
#[test]
fn test_result_operator() {
    use crate::Transaction;
    use crate::result::result;

    // Test context
    #[derive(Debug, Clone)]
    struct TestContext {
        value: i32,
    }

    // Create a result transaction
    let tx = result::<TestContext, _, _>(Ok::<_, String>(42));

    // Execute the transaction
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut ctx = TestContext { value: 10 };
            let result = tx.execute(&mut ctx).await;

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 42);
            // Context should be unchanged
            assert_eq!(ctx.value, 10);
        });
}

// Test for the with_context operator
#[test]
fn test_with_context_operator() {
    use crate::Transaction;
    use crate::operators::with_context;

    // Simple context
    #[derive(Debug, Clone)]
    struct TestContext {
        value: i32,
    }

    // Use 'static to resolve the lifetime issue
    let tx = with_context::<TestContext, _, _, _, _>(|ctx| {
        // Create a move block separately, capturing ctx by value
        let value = ctx.value + 5;
        async move { Ok::<_, String>(value) }
    });

    // Execute the transaction
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut ctx = TestContext { value: 10 };
            let result = tx.execute(&mut ctx).await;

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 15);
            // Context is now unchanged since we used the value directly
            assert_eq!(ctx.value, 10);
        });
}
