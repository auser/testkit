//! Integration tests for safe database name handling

#[cfg(test)]
mod safe_naming_tests {
    use db_testkit::test_db::DatabaseName;

    #[test]
    fn test_safe_database_name_generation() {
        // Generate a database name
        let name = DatabaseName::new(Some("testkit"));

        // The name should start with the prefix
        assert!(name.as_str().starts_with("testkit_"));

        // The name should not contain any hyphens (unsafe for MySQL)
        assert!(!name.as_str().contains('-'));

        // The name should only contain alphanumeric characters and underscores
        for c in name.as_str().chars() {
            assert!(c.is_alphanumeric() || c == '_');
        }

        tracing::info!("Generated safe database name: {}", name);
    }

    #[test]
    fn test_database_name_uniqueness() {
        // Generate multiple database names and ensure they're unique
        let names = (0..10)
            .map(|_| DatabaseName::new(Some("testkit")))
            .collect::<Vec<_>>();

        // Check all names are unique
        for i in 0..names.len() {
            for j in i + 1..names.len() {
                assert_ne!(
                    names[i].as_str(),
                    names[j].as_str(),
                    "Names should be unique: {} vs {}",
                    names[i],
                    names[j]
                );
            }
        }
    }

    #[test]
    fn test_database_name_custom_prefix() {
        // Generate a name with a custom prefix
        let name = DatabaseName::new(Some("custom"));

        // The name should start with the custom prefix
        assert!(name.as_str().starts_with("custom_"));

        // Generate a name with an empty prefix (should use default)
        let default_name = DatabaseName::new(None);

        // Should use a default prefix
        assert!(
            default_name.as_str().starts_with("testkit_")
                || default_name.as_str().starts_with("db_"),
            "Expected default prefix, got: {}",
            default_name
        );
    }
}
