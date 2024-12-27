use url::Url;

use crate::error::TestkitResult;

pub fn get_database_host_from_uri(database_uri: &str) -> TestkitResult<String> {
    let parsed = Url::parse(database_uri)?;
    let database_host = format!(
        "{}://{}:{}@{}:{}",
        parsed.scheme(),
        "mysql",
        parsed.password().unwrap_or(""),
        parsed.host_str().unwrap_or(""),
        parsed.port().unwrap_or(3306)
    );
    Ok(database_host)
}

pub fn drop_database_command(database_host: &str, database_name: &str) -> TestkitResult<()> {
    let output = Command::new("mysql")
        .arg(database_host)
        .arg("-e")
        .arg(&format!("DROP DATABASE IF EXISTS {database_name};"))
        .output()?;

    if !output.status.success() {
        return Err(TestkitError::DatabaseDropFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
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
    let output = Command::new("mysql")
        .arg(database_host)
        .arg("-e")
        .arg(&format!(
            "SELECT CONCAT('KILL ', id, ';') \
          FROM INFORMATION_SCHEMA.PROCESSLIST \
          WHERE db = '{}' \
          INTO OUTFILE '/tmp/kill.sql';
          SOURCE /tmp/kill.sql;
          SYSTEM rm /tmp/kill.sql;",
            database_name
        ))
        .output()?;

    if !output.status.success() {
        return Err(TestkitError::DatabaseDropFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn connect_to_db(database_uri: &str) -> TestkitResult<mysql_async::Conn> {
    let config = mysql_async::Config::from_url(database_uri)?;
    #[cfg(feature = "tracing")]
    tracing::info!("Connecting to database: {:?}", database_uri);

    for attempt in 0..3 {
        match mysql_async::Conn::new(config.clone()).await {
            Ok(conn) => {
                #[cfg(feature = "tracing")]
                tracing::info!("Connected to {database_uri}");
                return Ok(conn);
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
    let config = mysql_async::Config::from_url(database_uri)?;
    let pool = mysql_async::Pool::new(config);
    Ok(pool)
}

pub(crate) async fn create_test_user(database_uri: &str) -> TestkitResult<String> {
    let conn = connect_to_db(database_uri).await?;
    let test_user = format!("test_user_{}", get_next_count());

    let create_user_sql = format!(
        "CREATE USER IF NOT EXISTS '{}'@'%' IDENTIFIED BY 'test_password'",
        test_user
    );

    match conn.query_drop(&create_user_sql).await {
        Ok(_) => Ok(test_user),
        Err(e) => {
            tracing::error!("Failed to create or check test user: {:?}", e);
            Err(TestkitError::DatabaseOperationFailed(format!(
                "Failed to create or check test user: {}",
                e
            )))
        }
    }
}

pub(crate) fn generate_test_db_uri(
    opts: &TestPoolOptions,
    database_name: &str,
    test_user: &str,
) -> String {
    let port = opts.port.unwrap_or(3306);
    let host = opts.host.as_deref().unwrap_or("localhost");
    format!("mysql://{test_user}:test_password@{host}:{port}/{database_name}")
}
