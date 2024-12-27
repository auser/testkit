use deadpool_postgres::Pool;
use std::process::Command;
use std::str::FromStr;
use std::time::Duration;
use tokio_postgres::NoTls;
use url::Url;

use crate::error::{TestkitError, TestkitResult};

use super::{get_next_count, TestPoolOptions};

impl Default for TestPoolOptions {
    fn default() -> Self {
        Self {
            user: Some("postgres".to_string()),
            password: Some("testpassword".to_string()),
            host: Some("postgres".to_string()),
            port: Some(5432),
            database_name: None,
            tracing: false,
        }
    }
}

pub(crate) fn sync_drop_database(database_uri: &str) -> TestkitResult<()> {
    let parsed = Url::parse(database_uri)?;
    let database_name = parsed.path().split('/').last().unwrap_or("");

    let test_user = parsed.username();

    let database_host = format!(
        "{}://{}:{}@{}:{}",
        parsed.scheme(),
        "postgres", // Always use the postgres superuser for dropping
        parsed.password().unwrap_or(""),
        parsed.host_str().unwrap_or(""),
        parsed.port().unwrap_or(5432)
    );

    terminate_connections(&database_host, database_name)?;
    drop_database_command(&database_host, database_name)?;
    drop_role_command(&database_host, test_user)?;

    Ok(())
}

pub(crate) fn get_database_host_from_uri(database_uri: &str) -> TestkitResult<String> {
    let parsed = Url::parse(database_uri)?;
    let database_host = format!(
        "{}://{}:{}@{}:{}",
        parsed.scheme(),
        "postgres", // Always use the postgres superuser for dropping
        parsed.password().unwrap_or(""),
        parsed.host_str().unwrap_or(""),
        parsed.port().unwrap_or(5432)
    );
    Ok(database_host)
}

pub(crate) fn drop_role_command(database_host: &str, role_name: &str) -> TestkitResult<()> {
    let output = Command::new("psql")
        .arg(database_host)
        .arg("-c")
        .arg(&format!("DROP ROLE IF EXISTS {role_name};"))
        .output()?;

    if !output.status.success() {
        return Err(TestkitError::DatabaseDropFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn terminate_connections(database_host: &str, database_name: &str) -> TestkitResult<()> {
    let output = Command::new("psql")
      .arg(database_host)
      .arg("-c")
      .arg(&format!("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{database_name}';"))
      .output()?;

    if !output.status.success() {
        return Err(TestkitError::DatabaseDropFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn drop_database_command(database_host: &str, database_name: &str) -> TestkitResult<()> {
    let output = Command::new("psql")
        .arg(database_host)
        .arg("-c")
        .arg(&format!("DROP DATABASE {database_name};"))
        .output()?;

    if !output.status.success() {
        return Err(TestkitError::DatabaseDropFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

pub async fn connect_to_db(database_uri: &str) -> TestkitResult<tokio_postgres::Client> {
    let mut config = tokio_postgres::Config::from_str(database_uri)?;
    #[cfg(feature = "tracing")]
    tracing::info!("Connecting to database: {:?}", database_uri);

    for attempt in 0..3 {
        match config
            .connect_timeout(Duration::from_secs(5))
            .connect(NoTls)
            .await
        {
            Ok((client, connection)) => {
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("connection error: {}", e);
                    }
                });
                #[cfg(feature = "tracing")]
                tracing::info!("Connected to {database_uri}");
                return Ok(client);
            }
            Err(e) => {
                if attempt == 2 {
                    tracing::warn!("Failed to connect to database after 3 attempts: {}", e);
                    return Err(TestkitError::DatabaseConnectionFailed(e.to_string()));
                }
                tracing::warn!(
                    "Failed to connect to database, retrying in 1 second... Error: {}",
                    e
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    unreachable!()
}

pub(crate) fn create_connection_pool(database_uri: &str) -> TestkitResult<Pool> {
    use deadpool_postgres::{ManagerConfig, RecyclingMethod};

    let config = tokio_postgres::Config::from_str(database_uri)?;
    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };
    let manager = deadpool_postgres::Manager::from_config(config, NoTls, mgr_config);
    let pool = Pool::builder(manager)
        .max_size(16)
        .build()
        .map_err(|e| TestkitError::PoolCreationFailed(e.to_string()))?; // Explicitly handle the error conversion

    Ok(pool)
}

pub(crate) async fn grant_permissions(database_uri: &str, test_user: &str) -> TestkitResult<()> {
    let client = connect_to_db(database_uri).await?;

    // Use string interpolation instead of placeholders
    let grant_tables_sql = format!(
        "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO {}",
        test_user
    );
    client.execute(&grant_tables_sql, &[]).await?;

    let grant_sequences_sql = format!(
        "GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO {}",
        test_user
    );
    client.execute(&grant_sequences_sql, &[]).await?;

    let grant_schema_sql = format!("GRANT ALL PRIVILEGES ON SCHEMA public TO {}", test_user);
    client.execute(&grant_schema_sql, &[]).await?;

    Ok(())
}

pub async fn create_test_user(database_uri: &str) -> TestkitResult<String> {
    let test_user = format!("test_user_{}", get_next_count());
    let conn = create_connection_pool(database_uri)?;
    let client = conn.get().await?;

    // Create user if not exists
    client
        .execute(
            &format!(
                "DO $$ BEGIN 
                    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '{}') THEN
                        CREATE USER {} WITH PASSWORD '{}';
                    END IF;
                END $$;",
                test_user, test_user, test_user
            ),
            &[],
        )
        .await?;

    Ok(test_user)
}

pub(crate) fn generate_test_db_uri(
    opts: &TestPoolOptions,
    database_name: &str,
    test_user: &str,
) -> String {
    let port = opts.port.unwrap_or(5432);
    let host = opts.host.as_deref().unwrap_or("postgres");
    format!("postgresql://{test_user}:test_password@{host}:{port}/{database_name}?sslmode=disable")
}

#[cfg(test)]
mod tests {

    use crate::db::{
        create_database, drop_database, generate_database_name, TestPoolOptionsBuilder,
    };

    use super::*;

    #[tokio::test]
    async fn test_grant_permissions() {
        let database_name = generate_database_name();
        let opts = TestPoolOptions {
            user: Some("postgres".to_string()),
            password: Some("postgres".to_string()),
            host: Some("postgres".to_string()),
            port: Some(5432),
            database_name: Some(database_name.clone()),
            tracing: false,
        };

        let uri = opts.to_uri();
        let res = create_database(&uri).await;
        assert!(res.is_ok());
        // Create test user first
        println!("uri: {}", uri);
        let test_user = create_test_user(&uri).await.unwrap();
        let uri = generate_test_db_uri(&opts, &database_name, &test_user);

        // Test granting permissions
        let result = grant_permissions(&uri, &test_user).await;
        assert!(result.is_ok());

        // Clean up
        println!("dropping database: {}", uri);
        drop_database(&uri).await.unwrap();
    }

    #[tokio::test]
    async fn test_create_test_user() {
        let database_name = generate_database_name();
        let opts = TestPoolOptionsBuilder::new()
            .with_database_name(database_name.clone())
            .build();

        let uri = opts.to_uri();
        let res = create_database(&uri).await;
        // println!("uri: {}", res.unwrap_err());
        assert!(res.is_ok());

        // Test creating user
        let test_user = create_test_user(&uri).await.unwrap();
        assert!(test_user.starts_with("test_user_"));
        let opts = TestPoolOptionsBuilder::new()
            .with_database_name(uri)
            .with_user(test_user)
            .build();
        let uri = opts.to_uri();

        // Test idempotency
        let _ = create_test_user(&uri).await;

        // Clean up database but don't try to drop user
        drop_database(&uri).await.unwrap();
    }

    #[test]
    fn test_generate_test_db_uri() {
        let opts = TestPoolOptions {
            user: None,
            password: None,
            host: Some("testhost".to_string()),
            port: Some(5433),
            database_name: Some("test_db".to_string()),
            tracing: false,
        };

        let database_name = "test_db";
        let test_user = "test_user_1";

        let uri = generate_test_db_uri(&opts, database_name, test_user);
        assert_eq!(
            uri,
            "postgresql://test_user_1:test_password@testhost:5433/test_db?sslmode=disable"
        );

        // Test with default values
        let opts = TestPoolOptions {
            user: None,
            password: None,
            host: None,
            port: None,
            database_name: None,
            tracing: false,
        };

        let uri = generate_test_db_uri(&opts, database_name, test_user);
        assert_eq!(
            uri,
            "postgresql://test_user_1:test_password@postgres:5432/test_db?sslmode=disable"
        );
    }
}
