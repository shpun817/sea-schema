use super::{
    seaql_migrations, MigrationConnection, MigrationDbBackend, MigrationName, MigrationQueryResult,
    MigrationTrait, SchemaManager,
};
use sea_query::{
    Alias, ColumnDef, Condition, Expr, ForeignKey, IntoTableRef, Order, Query, SelectStatement,
    SimpleExpr, Table,
};
use std::fmt::Display;
use std::time::SystemTime;
use tracing::info;

#[derive(Debug, PartialEq)]
/// Status of migration
pub enum MigrationStatus {
    /// Not yet applied
    Pending,
    /// Applied
    Applied,
}

pub struct Migration<C>
where
    C: MigrationConnection,
{
    migration: Box<dyn MigrationTrait<C>>,
    status: MigrationStatus,
}

impl Display for MigrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = match self {
            MigrationStatus::Pending => "Pending",
            MigrationStatus::Applied => "Applied",
        };
        write!(f, "{}", status)
    }
}

/// Performing migrations on a database
#[async_trait::async_trait]
pub trait MigratorTrait: Send {
    type Conn: MigrationConnection;

    /// Vector of migrations in time sequence
    fn migrations() -> Vec<Box<dyn MigrationTrait<Self::Conn>>>;

    /// Get list of migrations wrapped in `Migration` struct
    fn get_migration_files() -> Vec<Migration<Self::Conn>> {
        Self::migrations()
            .into_iter()
            .map(|migration| Migration {
                migration,
                status: MigrationStatus::Pending,
            })
            .collect()
    }

    /// Get list of applied migrations from database
    async fn get_migration_models(
        db: &Self::Conn,
    ) -> Result<Vec<seaql_migrations::Model>, <Self::Conn as MigrationConnection>::Error> {
        Self::install(db).await?;
        let stmt = Query::select()
            .from(seaql_migrations::Table)
            .exprs([
                Expr::col(seaql_migrations::Column::Version),
                Expr::col(seaql_migrations::Column::AppliedAt),
            ])
            .order_by(seaql_migrations::Column::Version, Order::Asc)
            .to_owned();
        db.query_all(&stmt)
            .await?
            .into_iter()
            .map(|res| seaql_migrations::Model::try_from_query_result(res))
            .collect()
    }

    /// Get list of migrations with status
    async fn get_migration_with_status(
        db: &Self::Conn,
    ) -> Result<Vec<Migration<Self::Conn>>, <Self::Conn as MigrationConnection>::Error> {
        Self::install(db).await?;
        let mut migration_files = Self::get_migration_files();
        let migration_models = Self::get_migration_models(db).await?;
        for (i, migration_model) in migration_models.into_iter().enumerate() {
            if let Some(migration_file) = migration_files.get_mut(i) {
                if migration_file.migration.name() == migration_model.version.as_str() {
                    migration_file.status = MigrationStatus::Applied;
                } else {
                    return Err(Self::Conn::into_migration_error(format!("Migration mismatch: applied migration != migration file, '{0}' != '{1}'\nMigration '{0}' has been applied but its corresponding migration file is missing.", migration_file.migration.name(), migration_model.version)));
                }
            } else {
                return Err(Self::Conn::into_migration_error(format!("Migration file of version '{}' is missing, this migration has been applied but its file is missing", migration_model.version)));
            }
        }
        Ok(migration_files)
    }

    /// Get list of pending migrations
    async fn get_pending_migrations(
        db: &Self::Conn,
    ) -> Result<Vec<Migration<Self::Conn>>, <Self::Conn as MigrationConnection>::Error> {
        Self::install(db).await?;
        Ok(Self::get_migration_with_status(db)
            .await?
            .into_iter()
            .filter(|file| file.status == MigrationStatus::Pending)
            .collect())
    }

    /// Get list of applied migrations
    async fn get_applied_migrations(
        db: &Self::Conn,
    ) -> Result<Vec<Migration<Self::Conn>>, <Self::Conn as MigrationConnection>::Error> {
        Self::install(db).await?;
        Ok(Self::get_migration_with_status(db)
            .await?
            .into_iter()
            .filter(|file| file.status == MigrationStatus::Applied)
            .collect())
    }

    /// Create migration table `seaql_migrations` in the database
    async fn install(db: &Self::Conn) -> Result<(), <Self::Conn as MigrationConnection>::Error> {
        let stmt = Table::create()
            .if_not_exists()
            .table(seaql_migrations::Table)
            .col(
                ColumnDef::new(seaql_migrations::Column::Version)
                    .string()
                    .primary_key()
                    .not_null(),
            )
            .col(
                ColumnDef::new(seaql_migrations::Column::AppliedAt)
                    .big_integer()
                    .not_null(),
            )
            .to_owned();
        db.exec_stmt(&stmt).await
    }

    /// Drop all tables from the database, then reapply all migrations
    async fn fresh(db: &Self::Conn) -> Result<(), <Self::Conn as MigrationConnection>::Error> {
        Self::install(db).await?;
        let db_backend = db.get_database_backend();

        // Temporarily disable the foreign key check
        if db_backend == MigrationDbBackend::Sqlite {
            info!("Disabling foreign key check");
            db.exec_stmt(&"PRAGMA foreign_keys = OFF".to_owned())
                .await?;
            info!("Foreign key check disabled");
        }

        // Drop all foreign keys
        if db_backend == MigrationDbBackend::MySql {
            info!("Dropping all foreign keys");
            let mut stmt = Query::select();
            stmt.columns([Alias::new("TABLE_NAME"), Alias::new("CONSTRAINT_NAME")])
                .from((
                    Alias::new("information_schema"),
                    Alias::new("table_constraints"),
                ))
                .cond_where(
                    Condition::all()
                        .add(
                            Expr::expr(get_current_schema(db)).equals(
                                Alias::new("table_constraints"),
                                Alias::new("table_schema"),
                            ),
                        )
                        .add(Expr::expr(Expr::value("FOREIGN KEY")).equals(
                            Alias::new("table_constraints"),
                            Alias::new("constraint_type"),
                        )),
                );
            let rows = db.query_all(&stmt).await?;
            for row in rows.into_iter() {
                let constraint_name = row.try_get_string("CONSTRAINT_NAME")?;
                let table_name = row.try_get_string("TABLE_NAME")?;
                info!(
                    "Dropping foreign key '{}' from table '{}'",
                    constraint_name, table_name
                );
                let mut stmt = ForeignKey::drop();
                stmt.table(Alias::new(table_name.as_str()))
                    .name(constraint_name.as_str());
                db.exec_stmt(&stmt).await?;
                info!("Foreign key '{}' has been dropped", constraint_name);
            }
            info!("All foreign keys dropped");
        }

        // Drop all tables
        let stmt = query_tables(db);
        let rows = db.query_all(&stmt).await?;
        for row in rows.into_iter() {
            let table_name = row.try_get_string("table_name")?;
            info!("Dropping table '{}'", table_name);
            let mut stmt = Table::drop();
            stmt.table(Alias::new(table_name.as_str()))
                .if_exists()
                .cascade();
            db.exec_stmt(&stmt).await?;
            info!("Table '{}' has been dropped", table_name);
        }

        // Restore the foreign key check
        if db_backend == MigrationDbBackend::Sqlite {
            info!("Restoring foreign key check");
            db.exec_stmt(&"PRAGMA foreign_keys = ON".to_owned()).await?;
            info!("Foreign key check restored");
        }

        // Reapply all migrations
        Self::up(db, None).await
    }

    /// Rollback all applied migrations, then reapply all migrations
    async fn refresh(db: &Self::Conn) -> Result<(), <Self::Conn as MigrationConnection>::Error> {
        Self::down(db, None).await?;
        Self::up(db, None).await
    }

    /// Rollback all applied migrations
    async fn reset(db: &Self::Conn) -> Result<(), <Self::Conn as MigrationConnection>::Error> {
        Self::down(db, None).await
    }

    /// Check the status of all migrations
    async fn status(db: &Self::Conn) -> Result<(), <Self::Conn as MigrationConnection>::Error> {
        Self::install(db).await?;

        info!("Checking migration status");

        for Migration { migration, status } in Self::get_migration_with_status(db).await? {
            info!("Migration '{}'... {}", migration.name(), status);
        }

        Ok(())
    }

    /// Apply pending migrations
    async fn up(
        db: &Self::Conn,
        mut steps: Option<u32>,
    ) -> Result<(), <Self::Conn as MigrationConnection>::Error> {
        Self::install(db).await?;
        let manager = SchemaManager::new(db);

        if let Some(steps) = steps {
            info!("Applying {} pending migrations", steps);
        } else {
            info!("Applying all pending migrations");
        }

        let migrations = Self::get_pending_migrations(db).await?.into_iter();
        if migrations.len() == 0 {
            info!("No pending migrations");
        }
        for Migration { migration, .. } in migrations {
            if let Some(steps) = steps.as_mut() {
                if steps == &0 {
                    break;
                }
                *steps -= 1;
            }
            info!("Applying migration '{}'", migration.name());
            migration.up(&manager).await?;
            info!("Migration '{}' has been applied", migration.name());
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("SystemTime before UNIX EPOCH!");
            let stmt = Query::insert()
                .into_table(seaql_migrations::Table)
                .columns([
                    seaql_migrations::Column::Version,
                    seaql_migrations::Column::AppliedAt,
                ])
                .values_panic([migration.name().into(), (now.as_secs() as i64).into()])
                .to_owned();
            db.exec_stmt(&stmt).await?;
        }

        Ok(())
    }

    /// Rollback applied migrations
    async fn down(
        db: &Self::Conn,
        mut steps: Option<u32>,
    ) -> Result<(), <Self::Conn as MigrationConnection>::Error> {
        Self::install(db).await?;
        let manager = SchemaManager::new(db);

        if let Some(steps) = steps {
            info!("Rolling back {} applied migrations", steps);
        } else {
            info!("Rolling back all applied migrations");
        }

        let migrations = Self::get_applied_migrations(db).await?.into_iter().rev();
        if migrations.len() == 0 {
            info!("No applied migrations");
        }
        for Migration { migration, .. } in migrations {
            if let Some(steps) = steps.as_mut() {
                if steps == &0 {
                    break;
                }
                *steps -= 1;
            }
            info!("Rolling back migration '{}'", migration.name());
            migration.down(&manager).await?;
            info!("Migration '{}' has been rollbacked", migration.name());
            let stmt = Query::delete()
                .from_table(seaql_migrations::Table)
                .and_where(Expr::col(seaql_migrations::Column::Version).eq(migration.name()))
                .to_owned();
            db.exec_stmt(&stmt).await?;
        }

        Ok(())
    }
}

pub(crate) fn query_tables<C>(db: &C) -> SelectStatement
where
    C: MigrationConnection,
{
    let mut stmt = Query::select();
    let (expr, tbl_ref, condition) = match db.get_database_backend() {
        MigrationDbBackend::MySql => (
            Expr::col(Alias::new("table_name")),
            (Alias::new("information_schema"), Alias::new("tables")).into_table_ref(),
            Condition::all().add(
                Expr::expr(get_current_schema(db))
                    .equals(Alias::new("tables"), Alias::new("table_schema")),
            ),
        ),
        MigrationDbBackend::Postgres => (
            Expr::col(Alias::new("table_name")),
            (Alias::new("information_schema"), Alias::new("tables")).into_table_ref(),
            Condition::all()
                .add(
                    Expr::expr(get_current_schema(db))
                        .equals(Alias::new("tables"), Alias::new("table_schema")),
                )
                .add(Expr::col(Alias::new("table_type")).eq("BASE TABLE")),
        ),
        MigrationDbBackend::Sqlite => (
            Expr::col(Alias::new("name")),
            Alias::new("sqlite_master").into_table_ref(),
            Condition::all()
                .add(Expr::col(Alias::new("type")).eq("table"))
                .add(Expr::col(Alias::new("name")).ne("sqlite_sequence")),
        ),
    };
    stmt.expr_as(expr, Alias::new("table_name"))
        .from(tbl_ref)
        .cond_where(condition);
    stmt
}

pub(crate) fn get_current_schema<C>(db: &C) -> SimpleExpr
where
    C: MigrationConnection,
{
    match db.get_database_backend() {
        MigrationDbBackend::MySql => Expr::cust("DATABASE()"),
        MigrationDbBackend::Postgres => Expr::cust("CURRENT_SCHEMA()"),
        MigrationDbBackend::Sqlite => unimplemented!(),
    }
}
