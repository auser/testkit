//! Tests for operations that span multiple databases

use db_testkit::{
    error::Result,
    init_tracing,
    PoolConfig,
};

use tracing::info;

#[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
mod postgres_cross_db_tests {
    use sqlx::Executor;
    use super::*;

    #[tokio::test]
    async fn test_multiple_database_connections() -> Result<()> {
        init_tracing();
        info!("Testing multiple database connections");

        let connection_string = "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";

        // Create two separate test databases
        with_test_db!(connection_string, |db1| async move {
            // Set up first database with a table
            let mut conn1 = db1.connection().await?;
            conn1.execute("CREATE TABLE db1_table (id INT PRIMARY KEY, value TEXT)").await?;
            conn1.execute("INSERT INTO db1_table VALUES (1, 'db1_value')").await?;

            // Create a second test database
            with_test_db!(connection_string, |db2| async move {
                // Set up second database with a different table
                let mut conn2 = db2.connection().await?;
                conn2.execute("CREATE TABLE db2_table (id INT PRIMARY KEY, value TEXT)").await?;
                conn2.execute("INSERT INTO db2_table VALUES (2, 'db2_value')").await?;

                // Verify each database has its own data
                let rows1 = conn1.fetch_all("SELECT * FROM db1_table").await?;
                assert_eq!(rows1.len(), 1, "Expected 1 row in db1_table");

                let rows2 = conn2.fetch_all("SELECT * FROM db2_table").await?;
                assert_eq!(rows2.len(), 1, "Expected 1 row in db2_table");

                // Verify cross-database queries fail (as expected)
                let result = conn2.execute("SELECT * FROM db1_table").await;
                assert!(result.is_err(), "Cross-database query should fail");

                let result = conn1.execute("SELECT * FROM db2_table").await;
                assert!(result.is_err(), "Cross-database query should fail");

                info!("Multiple database connection test passed");
                Ok(())
            })
            .await
        })
        .await
    }

    #[tokio::test]
    async fn test_transaction_isolation() -> Result<()> {
        init_tracing();
        info!("Testing transaction isolation");

        with_test_db!(|db| async move {
            // Set up a test table
            let mut conn = db.connection().await?;
            conn.execute("CREATE TABLE isolation_test (id INT PRIMARY KEY, value TEXT)").await?;
            conn.execute("INSERT INTO isolation_test VALUES (1, 'initial')").await?;

            // Create two separate connections to the same database
            let mut conn1 = db.connection().await?;
            let mut conn2 = db.connection().await?;

            // Start a transaction on the first connection
            let mut tx1 = conn1.begin().await?;
            
            // Update the value in the transaction
            tx1.execute("UPDATE isolation_test SET value = 'updated' WHERE id = 1").await?;
            
            // Verify the change is visible inside the transaction
            let rows = tx1.fetch_all("SELECT * FROM isolation_test").await?;
            assert_eq!(rows[0].get::<String, _>("value"), "updated", "Transaction should see its own changes");
            
            // Verify the change is NOT visible to other connections (transaction isolation)
            let rows = conn2.fetch_all("SELECT * FROM isolation_test").await?;
            assert_eq!(rows[0].get::<String, _>("value"), "initial", "Second connection should not see uncommitted changes");
            
            // Commit the transaction
            tx1.commit().await?;
            
            // Now both connections should see the change
            let rows = conn2.fetch_all("SELECT * FROM isolation_test").await?;
            assert_eq!(rows[0].get::<String, _>("value"), "updated", "After commit, all connections should see changes");
            
            info!("Transaction isolation test passed");
            Ok(())
        })
        .await
    }
}

#[cfg(feature = "sqlx-sqlite")]
mod sqlite_cross_db_tests {
    use super::*;
    use sqlx::query;

    #[tokio::test]
    async fn test_sqlite_in_memory_isolation() -> Result<()> {
        init_tracing();
        info!("Testing SQLite in-memory database isolation");

        // Create two in-memory databases
        let backend1 = db_testkit::backends::sqlite::SqliteBackend::new(":memory:")
            .await
            .expect("Failed to create first SQLite backend");
            
        let backend2 = db_testkit::backends::sqlite::SqliteBackend::new(":memory:")
            .await
            .expect("Failed to create second SQLite backend");
            
        // Create test databases
        let test_db1 = db_testkit::test_db::TestDatabase::new(backend1, PoolConfig::default()).await?;
        let test_db2 = db_testkit::test_db::TestDatabase::new(backend2, PoolConfig::default()).await?;
        
        // Set up data in first database
        let conn1 = test_db1.connection().await?;
        let pool1 = conn1.sqlx_pool();
        
        query("CREATE TABLE test_table1 (id INTEGER PRIMARY KEY, value TEXT)")
            .execute(pool1)
            .await?;
        query("INSERT INTO test_table1 VALUES (1, 'db1_value')")
            .execute(pool1)
            .await?;
        
        // Set up data in second database
        let conn2 = test_db2.connection().await?;
        let pool2 = conn2.sqlx_pool();
        
        query("CREATE TABLE test_table2 (id INTEGER PRIMARY KEY, value TEXT)")
            .execute(pool2)
            .await?;
        query("INSERT INTO test_table2 VALUES (2, 'db2_value')")
            .execute(pool2)
            .await?;
        
        // Verify each database has its own data
        let rows1 = query("SELECT * FROM test_table1")
            .fetch_all(pool1)
            .await?;
        assert_eq!(rows1.len(), 1, "Expected 1 row in test_table1");
        
        let rows2 = query("SELECT * FROM test_table2")
            .fetch_all(pool2)
            .await?;
        assert_eq!(rows2.len(), 1, "Expected 1 row in test_table2");
        
        // Verify the second database doesn't have tables from the first
        let result = query("SELECT * FROM test_table1")
            .execute(pool2)
            .await;
        assert!(result.is_err(), "Second database should not see first database's tables");
        
        // Verify the first database doesn't have tables from the second
        let result = query("SELECT * FROM test_table2")
            .execute(pool1)
            .await;
        assert!(result.is_err(), "First database should not see second database's tables");
        
        info!("SQLite in-memory database isolation test passed");
        Ok(())
    }
}

#[cfg(any(feature = "mysql", feature = "sqlx-mysql"))]
mod mysql_cross_db_tests {
    use super::*;
    use sqlx::{Executor, query};

    #[tokio::test]
    async fn test_mysql_schema_isolation() -> Result<()> {
        init_tracing();
        info!("Testing MySQL schema isolation");

        let connection_string = "mysql://root@mysql:3306?timeout=30&connect_timeout=30&pool_timeout=30&ssl-mode=DISABLED";

        with_test_db!(connection_string, |db1| async move {
            // Set up first database with a table
            let conn1 = db1.connection().await?;
            query("CREATE TABLE db1_table (id INT PRIMARY KEY, value VARCHAR(255))")
                .execute(&conn1)
                .await?;
            query("INSERT INTO db1_table VALUES (1, 'db1_value')")
                .execute(&conn1)
                .await?;

            with_test_db!(connection_string, |db2| async move {
                // Set up second database with a different table
                let conn2 = db2.connection().await?;
                query("CREATE TABLE db2_table (id INT PRIMARY KEY, value VARCHAR(255))")
                    .execute(&conn2)
                    .await?;
                query("INSERT INTO db2_table VALUES (2, 'db2_value')")
                    .execute(&conn2)
                    .await?;

                // Store database names for verification
                let db1_name = db1.name().as_str();
                // let db2_name = db2.name().as_str();
                
                // MySQL allows cross-database references if you specify the schema
                // Verify we can query db1 from a connection to db2
                let cross_db_query = format!("SELECT * FROM {}.db1_table", db1_name);
                let result = query(&cross_db_query).execute(&conn2).await;
                
                // This can succeed or fail depending on MySQL permissions, but let's log the result
                match result {
                    Ok(_) => info!("Cross-database query succeeded (MySQL allows this with schema qualification)"),
                    Err(e) => info!("Cross-database query failed: {}", e),
                }
                
                // Verify direct table access still fails
                let result = query("SELECT * FROM db1_table").execute(&conn2).await;
                assert!(result.is_err(), "Unqualified cross-database query should fail");

                info!("MySQL schema isolation test completed");
                Ok(())
            })
            .await
        })
        .await
    }
} 