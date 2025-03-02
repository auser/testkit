#[cfg(feature = "tracing")]
mod tracing;

#[cfg(feature = "tracing")]
pub use tracing::*;

// #[cfg(feature = "env")]
pub mod prelude {
    // #[cfg(feature = "env")]
    // pub use crate::env::*;
    pub use crate::operators::*;
    pub use crate::result::result;
}

mod operators;
mod result;

pub use operators::*;
pub use result::*;

use std::future::Future;

// /// A boxed future that resolves to a Result
pub type BoxFuture<'a, T, E> =
    std::pin::Pin<Box<dyn Future<Output = std::result::Result<T, E>> + Send + 'a>>;

#[must_use]
#[async_trait::async_trait]
pub trait Transaction: Send + Sync {
    /// The context of the transaction
    type Context: Send + Sync;
    /// Return type of the transaction
    type Item: Send + Sync;
    /// Error type of the transaction
    type Error: Send + Sync;

    /// Execute the transaction
    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error>;

    /// Box the transaction
    fn boxed<'a>(self) -> Box<Self>
    where
        Self: Sized + 'a,
    {
        Box::new(self)
    }

    fn then<Fun, NextTx, NextItem>(self, f: Fun) -> Then<Self, Fun, NextTx>
    where
        Self: Sized,
        Fun: Fn(Self::Item) -> NextTx,
        NextTx: Transaction<Context = Self::Context, Error = Self::Error, Item = NextItem>,
    {
        then(self, f)
    }

    fn or_else<Fun, NextTx>(self, f: Fun) -> OrElse<Self, Fun, NextTx>
    where
        Self: Sized,
        Fun: Fn(Self::Error) -> NextTx + Send + Sync,
        NextTx: Transaction<Context = Self::Context, Error = Self::Error, Item = Self::Item>,
    {
        or_else(self, f)
    }

    fn setup<Fun, NextTx>(self, f: Fun) -> Setup<Self, Fun, NextTx>
    where
        Self: Sized,
        Fun: Fn(Result<Self::Item, Self::Error>) -> NextTx + Send + Sync,
        NextTx: Transaction<Context = Self::Context, Error = Self::Error, Item = Self::Item>,
    {
        setup(self, f)
    }
}

/// types than can be converted into transaction
pub trait IntoTransaction<Context> {
    type Tx: Transaction<Context = Context, Item = Self::Item, Error = Self::Error>;
    type Error;
    type Item;

    fn into_transaction(self) -> Self::Tx;
}

impl<Tx, Context> IntoTransaction<Context> for Tx
where
    Tx: Transaction<Context = Context>,
{
    type Tx = Tx;
    type Error = Tx::Error;
    type Item = Tx::Item;

    fn into_transaction(self) -> Self::Tx {
        self
    }
}

impl<Context, T, E> IntoTransaction<Context> for Result<T, E>
where
    T: Clone + Send + Sync,
    E: Clone + Send + Sync,
    Context: Clone + Send + Sync,
{
    type Tx = result::TxResult<Context, T, E>;
    type Error = E;
    type Item = T;

    fn into_transaction(self) -> Self::Tx {
        result::result(self)
    }
}

// Wrapper struct to provide Transaction impl for functions
pub struct FnTransaction<F, Context, T, E>(pub F, std::marker::PhantomData<(Context, T, E)>)
where
    F: Fn(&mut Context) -> Result<T, E>;

impl<F, Context, T, E> FnTransaction<F, Context, T, E>
where
    F: Fn(&mut Context) -> Result<T, E>,
{
    pub fn new(f: F) -> Self {
        Self(f, std::marker::PhantomData)
    }
}

#[async_trait::async_trait]
impl<F, Context, T, E> Transaction for FnTransaction<F, Context, T, E>
where
    F: Fn(&mut Context) -> Result<T, E> + Send + Sync,
    Context: Send + Sync,
    T: Send + Sync,
    E: Send + Sync,
{
    type Context = Context;
    type Item = T;
    type Error = E;

    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        (self.0)(ctx)
    }
}

/// Configuration for database connections
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    /// Connection string for admin operations (schema changes, etc.)
    pub admin_url: String,
    /// Connection string for regular operations
    pub user_url: String,
}

impl DatabaseConfig {
    /// Create a new configuration with explicit connection strings
    pub fn new(admin_url: impl Into<String>, user_url: impl Into<String>) -> Self {
        Self {
            admin_url: admin_url.into(),
            user_url: user_url.into(),
        }
    }

    /// Get a configuration from environment variables
    /// Uses ADMIN_DATABASE_URL and DATABASE_URL
    pub fn from_env() -> std::result::Result<Self, std::env::VarError> {
        let admin_url = std::env::var("ADMIN_DATABASE_URL")?;
        let user_url = std::env::var("DATABASE_URL")?;
        Ok(Self::new(admin_url, user_url))
    }
}

/// Database context that can be used with transactions
#[derive(Debug, Clone)]
pub struct DatabaseContext<Conn> {
    /// The actual database connection
    pub connection: Conn,
    /// Configuration used to establish the connection
    pub config: DatabaseConfig,
}

impl<Conn> DatabaseContext<Conn> {
    /// Create a new database context with a connection and configuration
    pub fn new(connection: Conn, config: DatabaseConfig) -> Self {
        Self { connection, config }
    }
}

/// Create a transaction with database context
///
/// Uses the configuration already in the context. To load from environment
/// variables, initialize your context with `DatabaseConfig::from_env()`.
///
/// # Example
///
/// ```rust,ignore
/// // With explicit config
/// let config = DatabaseConfig::new("postgres://admin@localhost/mydb", "postgres://user@localhost/mydb");
/// let conn = get_database_connection(&config.user_url);
/// let ctx = DatabaseContext::new(conn, config);
/// let tx = with_database(|ctx| async {
///     // Use ctx.connection to interact with the database
///     // ctx.config contains the connection strings if needed
///     Ok(())
/// });
/// ```
pub fn with_database<F, Fut, Conn, T, E>(
    f: F,
) -> impl Transaction<Context = DatabaseContext<Conn>, Item = T, Error = E>
where
    F: Fn(&mut DatabaseContext<Conn>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = std::result::Result<T, E>> + Send + 'static,
    Conn: Send + Sync + 'static,
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    with_context(f)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct TestConn {
        value: i32,
    }

    impl TestConn {
        fn value(&self) -> i32 {
            self.value
        }

        fn set_value(&mut self, value: i32) {
            self.value = value;
        }
    }

    // Mock database connection for testing
    #[derive(Debug, Clone)]
    struct MockDbConnection {
        #[allow(dead_code)]
        name: String,
    }

    impl MockDbConnection {
        fn new(name: impl Into<String>) -> Self {
            Self { name: name.into() }
        }

        #[allow(dead_code)]
        fn get_name(&self) -> &str {
            &self.name
        }
    }

    // Mock database error type
    #[derive(Debug)]
    enum MockDbError {
        #[allow(dead_code)]
        ConnectionError(String),
    }

    #[tokio::test]
    async fn test_with_database() {
        // Create config
        let config = DatabaseConfig::new(
            "postgres://admin@localhost/testdb",
            "postgres://user@localhost/testdb",
        );

        // Create mock connection
        let mock_connection = MockDbConnection::new("test_connection");

        // Create database context
        let mut ctx = DatabaseContext::new(mock_connection, config);

        // Create a simple transaction that returns the database URL
        let tx = with_context(move |ctx: &mut DatabaseContext<MockDbConnection>| {
            // Clone the data we need to avoid reference lifetime issues
            let url = ctx.config.user_url.clone();

            async move { Ok::<String, MockDbError>(url) }
        });

        // Execute the transaction
        let result = tx.execute(&mut ctx).await;

        // Verify the result
        assert_eq!(result.unwrap(), "postgres://user@localhost/testdb");
    }

    pub fn simple_tx<F>(
        f: F,
    ) -> impl Transaction<Context = TestConn, Item = TestConn, Error = ()> + std::fmt::Debug
    where
        F: Fn(&mut TestConn) -> Result<TestConn, ()> + Send + Sync + 'static,
    {
        type TestTx = dyn Fn(&mut TestConn) -> Result<TestConn, ()> + Send + Sync;
        struct SimpleTx(Arc<TestTx>);

        impl std::fmt::Debug for SimpleTx {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("SimpleTx").finish()
            }
        }

        #[async_trait::async_trait]
        impl Transaction for SimpleTx {
            type Context = TestConn;
            type Item = TestConn;
            type Error = ();

            async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
                (self.0)(ctx)
            }
        }

        SimpleTx(Arc::new(f))
    }

    #[tokio::test]
    async fn test_with_simple_tx() {
        let tx = simple_tx(|conn| {
            let value = conn.value() + 1;
            conn.set_value(value);
            Ok(conn.clone())
        });

        let mut conn = TestConn { value: 42 };
        let result = tx.execute(&mut conn).await;

        assert_eq!(result.unwrap().value, 43);
        assert_eq!(conn.value, 43);
    }

    #[tokio::test]
    async fn test_then_operator() {
        let tx1 = simple_tx(|conn| {
            let value = conn.value() + 1;
            conn.set_value(value);
            Ok(conn.clone())
        });

        let chained_tx = tx1.then(|prev_conn| {
            simple_tx(move |conn| {
                let new_value = prev_conn.value * 2;
                conn.set_value(new_value);
                Ok(conn.clone())
            })
        });

        let mut conn = TestConn { value: 5 };
        let result = chained_tx.execute(&mut conn).await;

        assert_eq!(result.unwrap().value, 12);
        assert_eq!(conn.value, 12);
    }

    #[tokio::test]
    async fn test_or_else_operator() {
        // Create a transaction that fails and chains with a fallback in one step
        let recovery_tx = simple_tx(|_conn| Err(())).or_else(|_err| {
            simple_tx(|conn| {
                let value = conn.value() * 3;
                conn.set_value(value);
                Ok(conn.clone())
            })
        });

        let mut conn = TestConn { value: 5 };
        let result = recovery_tx.execute(&mut conn).await;

        assert_eq!(result.unwrap().value, 15);
        assert_eq!(conn.value, 15);
    }

    #[tokio::test]
    async fn test_ok_operator() {
        let chained_tx = ok::<TestConn, TestConn, ()>(TestConn { value: 42 }).then(|prev_conn| {
            simple_tx(move |conn| {
                let new_value = prev_conn.value + 8;
                conn.set_value(new_value);
                Ok(conn.clone())
            })
        });

        let mut conn = TestConn { value: 10 };
        let result = chained_tx.execute(&mut conn).await;

        assert_eq!(result.unwrap().value, 50);
        assert_eq!(conn.value, 50);
    }

    #[tokio::test]
    async fn test_context_access() {
        // Create a transaction with simple_tx
        let tx = simple_tx(|conn| {
            let new_value = conn.value() * 5;
            conn.set_value(new_value);
            Ok(conn.clone())
        })
        .then(|prev_conn| {
            simple_tx(move |conn| {
                let new_value = prev_conn.value * 2;
                conn.set_value(new_value);
                Ok(conn.clone())
            })
        });

        let mut conn = TestConn { value: 7 };
        let result = tx.execute(&mut conn).await;

        // 7 * 5 = 35, then 35 * 2 = 70
        assert_eq!(result.unwrap().value, 70);
        assert_eq!(conn.value, 70);
    }

    #[tokio::test]
    async fn test_setup_operator() {
        // First test: setup handling a successful transaction
        {
            // First transaction adds 10 to the value
            let tx1 = simple_tx(|conn| {
                conn.set_value(conn.value() + 10);
                Ok(conn.clone())
            });

            // Setup checks if the first transaction succeeded and doubles value on success
            let setup_tx = tx1.setup(|res| {
                if res.is_ok() {
                    let conn = res.unwrap();
                    ok(TestConn {
                        value: conn.value * 2,
                    })
                } else {
                    ok(TestConn { value: 42 })
                }
            });

            let mut conn = TestConn { value: 5 };
            let result = setup_tx.execute(&mut conn).await;

            assert_eq!(result.unwrap().value, 30);
            assert_eq!(conn.value, 15);
        }

        // Second test: setup handling a failed transaction
        {
            let tx1 = simple_tx(|_| Err(()));

            let setup_tx = tx1.setup(|res| {
                if res.is_ok() {
                    let conn = res.unwrap();
                    ok(TestConn {
                        value: conn.value * 2,
                    })
                } else {
                    ok(TestConn { value: 42 })
                }
            });

            let mut conn = TestConn { value: 5 };
            let result = setup_tx.execute(&mut conn).await;

            assert_eq!(result.unwrap().value, 42);
            assert_eq!(conn.value, 5);
        }
    }
}
