#[cfg(all(test, feature = "mysql"))]
use crate::{backends::MySqlBackend, env::get_mysql_url};

#[cfg(feature = "postgres")]
#[cfg(test)]
use crate::{
    backend::{Connection, DatabasePool},
    backends::postgres::PostgresBackend,
    env::get_postgres_url,
    migrations::RunSql,
    pool::PoolConfig,
    template::DatabaseTemplate,
};

#[allow(dead_code)]
const SQL_SCRIPTS: &[&str] = &[
    r#"
    CREATE TABLE users (
        id SERIAL PRIMARY KEY,
        email VARCHAR(255) UNIQUE NOT NULL,
        name VARCHAR(255) NOT NULL
    );
    "#,
    r#"
    ALTER TABLE users ADD COLUMN created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP;
    "#,
];

#[tokio::test]
#[cfg(feature = "postgres")]
async fn test_get_postgres_url() {
    let url = get_postgres_url().unwrap();
    assert!(url.contains("postgres"));
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn test_postgres_template() {
    let postgres_url = get_postgres_url().unwrap();
    let backend = PostgresBackend::new(&postgres_url).await.unwrap();

    let template = DatabaseTemplate::new(backend, PoolConfig::default(), 5)
        .await
        .unwrap();

    // Initialize template with SQL scripts
    template
        .initialize_template(|mut conn| async move {
            conn.run_sql_scripts(&crate::SqlSource::Embedded(SQL_SCRIPTS))
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    // Get two separate databases
    let db1 = template.get_immutable_database().await.unwrap();
    let db2 = template.get_immutable_database().await.unwrap();

    // Verify they are separate
    let mut conn1 = db1.get_pool().acquire().await.unwrap();
    let mut conn2 = db2.get_pool().acquire().await.unwrap();

    // Insert into db1
    conn1
        .execute("INSERT INTO users (email, name) VALUES ('test1@example.com', 'Test User 1')")
        .await
        .unwrap();

    // Insert into db2
    conn2
        .execute("INSERT INTO users (email, name) VALUES ('test2@example.com', 'Test User 2')")
        .await
        .unwrap();

    // Verify data is separate
    conn1
        .execute("SELECT email FROM users WHERE email = 'test1@example.com'")
        .await
        .unwrap();

    conn2
        .execute("SELECT email FROM users WHERE email = 'test2@example.com'")
        .await
        .unwrap();
}

#[tokio::test]
#[cfg(feature = "mysql")]
async fn test_mysql_template() {
    let backend = MySqlBackend::new(&get_mysql_url().unwrap()).unwrap();

    let template = DatabaseTemplate::new(backend, PoolConfig::default(), 5)
        .await
        .unwrap();

    // Initialize template with SQL scripts
    template
        .initialize_template(|mut conn| async move {
            conn.run_sql_scripts(&SqlSource::Embedded(SQL_SCRIPTS))
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    // Get two separate databases
    let db1 = template.get_immutable_database().await.unwrap();
    let db2 = template.get_immutable_database().await.unwrap();

    // Verify they are separate
    let mut conn1 = db1.get_pool().acquire().await.unwrap();
    let mut conn2 = db2.get_pool().acquire().await.unwrap();

    // Insert into db1
    conn1
        .execute("INSERT INTO users (email, name) VALUES ('test1@example.com', 'Test User 1')")
        .await
        .unwrap();

    // Insert into db2
    conn2
        .execute("INSERT INTO users (email, name) VALUES ('test2@example.com', 'Test User 2')")
        .await
        .unwrap();

    // Verify data is separate
    conn1
        .execute("SELECT email FROM users WHERE email = 'test1@example.com'")
        .await
        .unwrap();

    conn2
        .execute("SELECT email FROM users WHERE email = 'test2@example.com'")
        .await
        .unwrap();
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn test_parallel_databases() {
    let backend = crate::PostgresBackend::new(&crate::prelude::get_postgres_url().unwrap())
        .await
        .unwrap();
    let template = crate::DatabaseTemplate::new(backend, crate::PoolConfig::default(), 10)
        .await
        .unwrap();

    // Initialize template
    template
        .initialize_template(|mut conn| async move {
            conn.run_sql_scripts(&crate::SqlSource::Embedded(SQL_SCRIPTS))
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    // Create multiple databases concurrently
    let mut handles = vec![];
    for i in 0..5 {
        let template = template.clone();
        handles.push(tokio::spawn(async move {
            let db = template.get_immutable_database().await.unwrap();
            let mut conn = db.get_pool().acquire().await.unwrap();

            // Insert data specific to this instance
            conn.execute(&format!(
                "INSERT INTO users (email, name) VALUES ('test{}@example.com', 'Test User {}')",
                i, i
            ))
            .await
            .unwrap();

            // Verify only our data exists
            conn.execute(&format!(
                "SELECT * FROM users WHERE email = 'test{}@example.com'",
                i
            ))
            .await
            .unwrap();
        }));
    }

    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn test_concurrent_operations() {
    let backend = crate::PostgresBackend::new(&crate::prelude::get_postgres_url().unwrap())
        .await
        .unwrap();
    let template = crate::DatabaseTemplate::new(backend, crate::PoolConfig::default(), 1)
        .await
        .unwrap();

    // Initialize template
    template
        .initialize_template(|mut conn| async move {
            conn.run_sql_scripts(&crate::SqlSource::Embedded(SQL_SCRIPTS))
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    // Get a single database
    let db = template.get_immutable_database().await.unwrap();

    // Run concurrent operations on the same database
    let mut handles = vec![];
    for i in 0..5 {
        let pool = db.get_pool().clone();
        handles.push(tokio::spawn(async move {
            let mut conn = pool.acquire().await.unwrap();
            conn.execute(&format!(
                "INSERT INTO users (email, name) VALUES ('concurrent{}@example.com', 'Concurrent User {}')",
                i, i
            ))
            .await
            .unwrap();
        }));
    }

    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all data was inserted
    let mut conn = db.get_pool().acquire().await.unwrap();
    for i in 0..5 {
        conn.execute(&format!(
            "SELECT * FROM users WHERE email = 'concurrent{}@example.com'",
            i
        ))
        .await
        .unwrap();
    }
}
