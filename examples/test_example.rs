use db_testkit::{with_test_db, PostgresPool};

async fn get_organization(conn: &mut impl DatabaseConnection) -> Result<String, <impl DatabaseConnection as DatabaseConnection>::Error> {
    // Example implementation
    conn.execute("SELECT 'Test Organization'").await?;
    Ok("Test Organization".to_string())
}

#[tokio::test]
async fn test_get_organization() {
    with_test_db!(PostgresPool, |mut conn| async move {
        // Setup the database
        conn.execute("CREATE TABLE organizations (id SERIAL PRIMARY KEY, name TEXT)")
            .await?;
        conn.execute("INSERT INTO organizations (name) VALUES ('Test Organization')")
            .await?;
        Ok(())
    }, |db| async move {
        // Get a connection from the pool
        let mut conn = db.connection().await.unwrap();
        let org = get_organization(&mut conn).await;
        assert!(org.is_ok());
        
        // Test with a transaction
        let mut tx = db.begin().await.unwrap();
        tx.execute("INSERT INTO organizations (name) VALUES ('Another Org')").await.unwrap();
        
        // We can roll back or commit
        tx.commit().await.unwrap();
    }).await;
    // Database will be automatically dropped here
} 