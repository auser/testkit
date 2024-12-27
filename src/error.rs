use deadpool_postgres::PoolError;

pub type TestkitResult<T = (), E = TestkitError> = Result<T, E>;

#[derive(Debug, Clone)]
pub enum TestkitError {
    IOError(String),
    EnvironmentError(String),
    PostgresError(String),
    MysqlError(String),

    UriParseError(String),

    DatabaseAlreadyExists(String),
    DatabaseCreationFailed,
    DatabaseDropFailed(String),
    DatabaseSetupFailed(String),

    DatabaseOperationFailed(String),
    MigrationFailed(String),
    PoolCreationFailed(String),
    DatabaseConnectionFailed(String),
}

impl std::fmt::Display for TestkitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestkitError::IOError(e) => write!(f, "IO error: {}", e),
            TestkitError::EnvironmentError(e) => write!(f, "Environment error: {}", e),
            TestkitError::PostgresError(e) => write!(f, "Postgres error: {}", e),
            TestkitError::MysqlError(e) => write!(f, "Mysql error: {}", e),
            #[cfg(feature = "postgres")]
            TestkitError::DatabaseDropFailed(e) => {
                write!(f, "Postgres database drop failed: {}", e)
            }
            #[cfg(feature = "mysql")]
            TestkitError::DatabaseDropFailed(e) => {
                write!(f, "Mysql database drop failed: {}", e)
            }
            TestkitError::DatabaseSetupFailed(e) => write!(f, "Database setup failed: {}", e),
            TestkitError::UriParseError(e) => write!(f, "URI parse error: {}", e),
            TestkitError::DatabaseOperationFailed(e) => {
                write!(f, "Database operation failed: {}", e)
            }
            TestkitError::MigrationFailed(e) => write!(f, "Migration failed: {}", e),
            TestkitError::PoolCreationFailed(e) => write!(f, "Pool creation failed: {}", e),
            TestkitError::DatabaseConnectionFailed(e) => {
                write!(f, "Database connection failed: {}", e)
            }
            TestkitError::DatabaseAlreadyExists(e) => write!(f, "Database already exists: {}", e),
            TestkitError::DatabaseCreationFailed => write!(f, "Database creation failed"),
        }
    }
}

impl From<deadpool_postgres::PoolError> for TestkitError {
    fn from(e: PoolError) -> Self {
        match e {
            PoolError::Backend(e) => TestkitError::PostgresError(e.to_string()),
            PoolError::Closed => TestkitError::PostgresError("Pool closed".to_string()),
            _ => TestkitError::PostgresError("database setup failed".to_string()),
        }
    }
}

impl From<url::ParseError> for TestkitError {
    fn from(e: url::ParseError) -> Self {
        TestkitError::UriParseError(e.to_string())
    }
}

impl From<std::io::Error> for TestkitError {
    fn from(e: std::io::Error) -> Self {
        TestkitError::IOError(e.to_string())
    }
}

impl From<std::env::VarError> for TestkitError {
    fn from(e: std::env::VarError) -> Self {
        TestkitError::EnvironmentError(e.to_string())
    }
}

impl From<tokio_postgres::Error> for TestkitError {
    fn from(e: tokio_postgres::Error) -> Self {
        TestkitError::DatabaseDropFailed(e.to_string())
    }
}
