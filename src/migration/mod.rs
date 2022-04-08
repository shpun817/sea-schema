pub mod connection;
pub mod error;
pub mod manager;
pub mod migrator;
pub mod prelude;
pub mod seaql_migrations;
pub mod statement;

pub use async_std;
pub use async_trait;
pub use connection::*;
pub use error::*;
pub use manager::*;
pub use migrator::*;
pub use statement::*;

pub trait MigrationName {
    fn name(&self) -> &str;
}

/// The migration definition
#[async_trait::async_trait]
pub trait MigrationTrait<C>: MigrationName + Send + Sync
where
    C: MigrationConnection,
{
    /// Define actions to perform when applying the migration
    async fn up(&self, manager: &SchemaManager<C>) -> Result<(), C::Error>;

    /// Define actions to perform when rolling back the migration
    async fn down(&self, manager: &SchemaManager<C>) -> Result<(), C::Error>;
}
