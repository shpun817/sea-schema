use super::{
    query_tables, MigrationConnection, MigrationDbBackend, MigrationQueryResult,
    MigrationStatementBuilder,
};
use sea_query::{
    extension::postgres::{TypeAlterStatement, TypeCreateStatement, TypeDropStatement},
    Alias, Condition, Expr, ForeignKeyCreateStatement, ForeignKeyDropStatement,
    IndexCreateStatement, IndexDropStatement, Query, TableAlterStatement, TableCreateStatement,
    TableDropStatement, TableRenameStatement, TableTruncateStatement,
};

/// Helper struct for writing migration scripts in migration file
pub struct SchemaManager<'c, C>
where
    C: MigrationConnection,
{
    conn: &'c C,
}

impl<'c, C> SchemaManager<'c, C>
where
    C: MigrationConnection,
{
    pub fn new(conn: &'c C) -> Self {
        Self { conn }
    }

    pub async fn exec_stmt<S>(&self, stmt: S) -> Result<(), C::Error>
    where
        S: MigrationStatementBuilder + Sync,
    {
        self.conn.exec_stmt(&stmt).await
    }

    pub fn get_database_backend(&self) -> MigrationDbBackend {
        self.conn.get_database_backend()
    }

    pub fn get_connection(&self) -> &'c C::Connection {
        self.conn.get_connection()
    }
}

/// Schema Creation
impl<'c, C> SchemaManager<'c, C>
where
    C: MigrationConnection,
{
    pub async fn create_table(&self, stmt: TableCreateStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn create_index(&self, stmt: IndexCreateStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn create_foreign_key(
        &self,
        stmt: ForeignKeyCreateStatement,
    ) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn create_type(&self, stmt: TypeCreateStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }
}

/// Schema Mutation
impl<'c, C> SchemaManager<'c, C>
where
    C: MigrationConnection,
{
    pub async fn alter_table(&self, stmt: TableAlterStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn drop_table(&self, stmt: TableDropStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn rename_table(&self, stmt: TableRenameStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn truncate_table(&self, stmt: TableTruncateStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn drop_index(&self, stmt: IndexDropStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn drop_foreign_key(&self, stmt: ForeignKeyDropStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn alter_type(&self, stmt: TypeAlterStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }

    pub async fn drop_type(&self, stmt: TypeDropStatement) -> Result<(), C::Error> {
        self.conn.exec_stmt(&stmt).await
    }
}

/// Schema Inspection
impl<'c, C> SchemaManager<'c, C>
where
    C: MigrationConnection,
{
    pub async fn has_table<T>(&self, table: T) -> Result<bool, C::Error>
    where
        T: AsRef<str>,
    {
        let mut stmt = Query::select();
        let mut subquery = query_tables(self.conn);
        subquery.cond_where(Expr::col(Alias::new("table_name")).eq(table.as_ref()));
        stmt.expr_as(Expr::cust("COUNT(*)"), Alias::new("rows"))
            .from_subquery(subquery, Alias::new("subquery"));

        let res = self
            .conn
            .query_one(&stmt)
            .await?
            .ok_or_else(|| C::into_migration_error("Fail to check table exists".to_owned()))?;
        let rows = res.try_get_i64("rows")?;

        Ok(rows > 0)
    }

    pub async fn has_column<TBL, COL>(&self, table: TBL, column: COL) -> Result<bool, C::Error>
    where
        TBL: AsRef<str>,
        COL: AsRef<str>,
    {
        let db_backend = self.conn.get_database_backend();
        let found = match db_backend {
            MigrationDbBackend::MySql | MigrationDbBackend::Postgres => {
                let schema_name = match db_backend {
                    MigrationDbBackend::MySql => "DATABASE()",
                    MigrationDbBackend::Postgres => "CURRENT_SCHEMA()",
                    MigrationDbBackend::Sqlite => unreachable!(),
                };
                let mut stmt = Query::select();
                stmt.expr_as(Expr::cust("COUNT(*)"), Alias::new("rows"))
                    .from((Alias::new("information_schema"), Alias::new("columns")))
                    .cond_where(
                        Condition::all()
                            .add(
                                Expr::expr(Expr::cust(schema_name))
                                    .equals(Alias::new("columns"), Alias::new("table_schema")),
                            )
                            .add(Expr::col(Alias::new("table_name")).eq(table.as_ref()))
                            .add(Expr::col(Alias::new("column_name")).eq(column.as_ref())),
                    );

                let res = self.conn.query_one(&stmt).await?.ok_or_else(|| {
                    C::into_migration_error("Fail to check column exists".to_owned())
                })?;
                let rows = res.try_get_i64("rows")?;
                rows > 0
            }
            MigrationDbBackend::Sqlite => {
                let stmt = format!("PRAGMA table_info({})", table.as_ref());
                let results = self.conn.query_all(&stmt).await?;
                let mut found = false;
                for res in results {
                    let name = res.try_get_string("name")?;
                    if name.as_str() == column.as_ref() {
                        found = true;
                    }
                }
                found
            }
        };
        Ok(found)
    }
}
