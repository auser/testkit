use std::future::Future;

use crate::db::{TestDatabase, TestPoolOptions};

pub async fn with_test_db<F, Fut, T>(test: F)
where
    F: FnOnce(TestDatabase) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error>>> + Send + 'static,
    T: Send + 'static,
{
    catch_panics();

    match TestDatabase::new(None).await {
        Ok(db) => {
            if let Err(err) = test(db).await {
                panic!("Test failed: {:?}", err);
            }
        }
        Err(e) => {
            panic!("Failed to create test database: {:?}", e);
        }
    }
}

pub async fn with_configured_test_db<F, Fut>(opts: TestPoolOptions, test: F)
where
    F: for<'a> FnOnce(TestDatabase) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), Box<dyn std::error::Error>>> + 'static + Send,
{
    // To catch panics
    catch_panics();

    let exec = TestDatabase::new(Some(opts))
        .await
        .expect("unable to get test db");
    if let Err(err) = test(exec).await {
        panic!("test failed: {:?}", err);
    }
}

fn catch_panics() {
    // To catch panics
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        // TODO: close down databases?
    }));
}

#[cfg(test)]
mod tests {
    use crate::db::TestDatabaseTrait;

    use super::*;

    struct TestEnv {
        test_user: String,
    }
    impl TestDatabaseTrait for TestEnv {}
    impl TestEnv {
        fn new(test_user: String) -> Self {
            Self { test_user }
        }
        // async fn setup(&self, _client: &mut Client) -> CommonResult<()> {
        //     Ok(())
        // }
    }

    #[tokio::test]
    async fn test_with_a_database() {
        with_test_db(|db: TestDatabase| async move {
            let test_user = db.test_user.clone();
            let env = db
                .setup(|mut client| async move { 
                    let env = TestEnv::new(test_user.clone());
                    
                    // Insert the user with the test_user as the email
                    let txn = client.transaction().await?;
                    txn.execute(
                        "INSERT INTO users (email, openid_sub, first_name, last_name, system_admin) VALUES ($1, $2, $3, $4, $5)",
                        &[&test_user, &"1234567890", &"Test", &"User", &true],
                    ).await?;
                    txn.commit().await?;    
                    Ok(env)
                })
                .await.expect("failed to setup test env");
    
            // Use the test_pool for querying
            let mut client = db.test_pool.get().await?;
            let txn = client.transaction().await?;
            
            // Set the role to the test user
            txn.execute(&format!("SET LOCAL row_level_security.user_id = '{}'", 1), &[]).await?;
    
            let rows = txn.query("SELECT * FROM users WHERE email = $1", &[&env.test_user]).await?;
    
            assert_eq!(rows.len(), 1, "Expected 1 user, found {}", rows.len());
            txn.commit().await?;
    
            Ok(())
        })
        .await;
    }

    #[tokio::test]
    async fn test_with_a_database_with_rls_enabled() {
        with_test_db(|db: TestDatabase| async move {
            let test_user = db.test_user.clone();
            let _env = db
                .setup(|mut client| async move { 
                    let env = TestEnv::new(test_user.clone());
                    
                    // Insert the user with the test_user as the email
                    let txn = client.transaction().await?;
                    txn.execute(
                        "INSERT INTO users (email, openid_sub, first_name, last_name, system_admin) VALUES ($1, $2, $3, $4, $5)",
                        &[&test_user, &"1234567890", &"Test", &"User", &false],
                    ).await?;
                    txn.commit().await?;    
                    Ok(env)
                })
                .await.expect("failed to setup test env");
    
            // Use the test_pool for querying
            let mut client = db.test_pool.get().await?;
            let txn = client.transaction().await?;
            
            let rows = txn.query("SELECT * FROM users WHERE email = 'bob@example.com'", &[]).await;
            assert!(rows.is_ok());
            let rows = rows.unwrap();
            assert_eq!(rows.len(), 0, "Expected 0 users, found {}", rows.len());
    
            Ok(())
        })
        .await;
    }
}
