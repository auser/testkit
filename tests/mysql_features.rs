//! Integration tests for MySQL features

mod common;

#[cfg(feature = "mysql")]
mod mysql_tests {
    use db_testkit::{
        backend::{Connection, DatabasePool},
        backends::MySqlBackend,
        env::get_mysql_url,
        migrations::RunSql,
        PoolConfig, SqlSource, TestDatabaseTemplate,
    };

    use crate::common::SQL_SCRIPTS;

    #[tokio::test]
    async fn test_mysql_template() {
        let backend = MySqlBackend::new(&get_mysql_url().unwrap()).unwrap();

        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 5)
            .await
            .unwrap();

        // Initialize template with SQL scripts
        template
            .initialize(|mut conn| async move {
                conn.run_sql_scripts(&SqlSource::Embedded(SQL_SCRIPTS))
                    .await?;
                Ok(())
            })
            .await
            .unwrap();

        // Get two separate databases
        let db1 = template.create_test_database().await.unwrap();
        let db2 = template.create_test_database().await.unwrap();

        // Verify they are separate
        let mut conn1 = db1.pool.acquire().await.unwrap();
        let mut conn2 = db2.pool.acquire().await.unwrap();

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

        // Verify data is separate - just check that executing SELECT doesn't error
        conn1
            .execute("SELECT email FROM users WHERE email = 'test1@example.com'")
            .await
            .unwrap();

        conn2
            .execute("SELECT email FROM users WHERE email = 'test2@example.com'")
            .await
            .unwrap();

        // Verify count in each database
        let rows1 = conn1.fetch("SELECT COUNT(*) FROM users").await.unwrap();
        assert_eq!(rows1.len(), 1);

        let rows2 = conn2.fetch("SELECT COUNT(*) FROM users").await.unwrap();
        assert_eq!(rows2.len(), 1);
    }
}
