use sea_orm::migration::*;
use sea_schema::migration::prelude::*;

mod m20220118_000001_create_cake_table;
mod m20220118_000002_create_fruit_table;
mod m20220118_000003_seed_cake_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    type Connection = DbConn;

    fn migrations() -> Vec<Box<dyn MigrationTrait<DbConn>>> {
        vec![
            Box::new(m20220118_000001_create_cake_table::Migration),
            Box::new(m20220118_000002_create_fruit_table::Migration),
            Box::new(m20220118_000003_seed_cake_table::Migration),
        ]
    }
}
