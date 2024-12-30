use uuid::Uuid;

pub fn get_db_name() -> String {
    format!(
        "testkit_db_{}",
        Uuid::new_v4().to_string().replace('-', "_")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_db_name() {
        let db_name = get_db_name();
        assert!(
            db_name.starts_with("testkit_db_"),
            "db_name should start with testkit_db_"
        );
    }
}
