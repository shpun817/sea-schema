pub trait IntoMigrationError {
    fn into_migration_error(str: String) -> Self;
}
