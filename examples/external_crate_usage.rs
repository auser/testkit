#[cfg(not(feature = "sqlx-postgres"))]
#[allow(unused_imports)]
use db_testkit::backends::PostgresBackend;
#[cfg(feature = "sqlx-postgres")]
use db_testkit::backends::SqlxPostgresBackend;

#[tokio::test]
async fn test_external_crate_usage() {
    // Method 1: Using the macro with type annotation
    #[cfg(feature = "sqlx-postgres")]
    with_test_db!(|db: TestDatabase<SqlxPostgresBackend>| async move {
        // Get a connection from the pool
        let mut conn = db.test_pool.acquire().await.unwrap();

        // Execute a query
        conn.execute("CREATE TABLE test_table (id SERIAL PRIMARY KEY, name TEXT)")
            .await
            .unwrap();

        conn.execute("INSERT INTO test_table (name) VALUES ('test')")
            .await
            .unwrap();

        // Verify the data
        let result = conn
            .fetch_one("SELECT name FROM test_table WHERE id = 1")
            .await
            .unwrap();

        let name: String = result.get("name");
        assert_eq!(name, "test");

        Ok(())
    });

    #[cfg(not(feature = "sqlx-postgres"))]
    with_test_db!(|db: TestDatabase<PostgresBackend>| async move {
        // Get a connection from the pool
        let mut conn = db.pool.acquire().await.unwrap();

        // Execute a query
        conn.execute("CREATE TABLE test_table (id SERIAL PRIMARY KEY, name TEXT)")
            .await
            .unwrap();

        conn.execute("INSERT INTO test_table (name) VALUES ('test')")
            .await
            .unwrap();

        Ok(())
    });

    // Method 2: Using the macro with custom URL
    #[cfg(feature = "sqlx-postgres")]
    with_test_db!(
        "postgres://postgres:postgres@localhost:5432/postgres?sslmode=disable",
        |db: TestDatabase<SqlxPostgresBackend>| async move {
            // Get a connection from the pool
            let mut conn = db.test_pool.acquire().await.unwrap();

            // Execute a query
            conn.execute("CREATE TABLE another_table (id SERIAL PRIMARY KEY, value TEXT)")
                .await
                .unwrap();

            Ok(())
        }
    );

    #[cfg(not(feature = "sqlx-postgres"))]
    with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
        |db: TestDatabase<PostgresBackend>| async move {
            // Get a connection from the pool
            let mut conn = db.pool.acquire().await.unwrap();

            // Execute a query
            conn.execute("CREATE TABLE another_table (id SERIAL PRIMARY KEY, value TEXT)")
                .await
                .unwrap();

            Ok(())
        }
    );
}

fn main() {
    // This example is meant to be run with cargo test
}
