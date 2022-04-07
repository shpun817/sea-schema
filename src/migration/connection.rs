use super::{IntoMigrationError, MigrationStatementBuilder};

/// The type of database backend for real world databases.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MigrationDbBackend {
    /// A MySQL backend
    MySql,
    /// A PostgreSQL backend
    Postgres,
    /// A SQLite backend
    Sqlite,
}

#[async_trait::async_trait]
pub trait MigrationConnection: Sync {
    type Connection;

    type QueryResult: MigrationQueryResult<Error = Self::Error> + Send;

    type Error: IntoMigrationError;

    async fn query_one<S>(&self, stmt: &S) -> Result<Option<Self::QueryResult>, Self::Error>
    where
        S: MigrationStatementBuilder + Sync;

    async fn query_all<S>(&self, stmt: &S) -> Result<Vec<Self::QueryResult>, Self::Error>
    where
        S: MigrationStatementBuilder + Sync;

    async fn exec_stmt<S>(&self, stmt: &S) -> Result<(), Self::Error>
    where
        S: MigrationStatementBuilder + Sync;

    fn get_database_backend(&self) -> MigrationDbBackend;

    fn get_connection(&self) -> &Self::Connection;

    fn into_migration_error(str: String) -> Self::Error {
        <Self::Error as IntoMigrationError>::into_migration_error(str)
    }
}

pub trait MigrationQueryResult: Sized {
    type Error: IntoMigrationError;

    fn try_get_i64(&self, col: &str) -> Result<i64, Self::Error>;

    fn try_get_string(&self, col: &str) -> Result<String, Self::Error>;
}
