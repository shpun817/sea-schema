use super::MigrationDbBackend;
use sea_query::{MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder, Value};

pub trait MigrationStatementBuilder {
    fn build(&self, db_backend: &MigrationDbBackend) -> (String, Vec<Value>);
}

macro_rules! build_any_stmt {
    ($stmt: expr, $db_backend: expr) => {
        match $db_backend {
            MigrationDbBackend::MySql => $stmt.build(MysqlQueryBuilder),
            MigrationDbBackend::Postgres => $stmt.build(PostgresQueryBuilder),
            MigrationDbBackend::Sqlite => $stmt.build(SqliteQueryBuilder),
        }
    };
}

macro_rules! build_postgres_stmt {
    ($stmt: expr, $db_backend: expr) => {
        match $db_backend {
            MigrationDbBackend::Postgres => $stmt.build(PostgresQueryBuilder),
            MigrationDbBackend::MySql | MigrationDbBackend::Sqlite => unimplemented!(),
        }
    };
}

macro_rules! build_query_stmt {
    ($stmt: ty) => {
        impl MigrationStatementBuilder for $stmt {
            fn build(&self, db_backend: &MigrationDbBackend) -> (String, Vec<Value>) {
                let (stmt, values) = build_any_stmt!(self, db_backend);
                (stmt, values.0)
            }
        }
    };
}

build_query_stmt!(sea_query::InsertStatement);
build_query_stmt!(sea_query::SelectStatement);
build_query_stmt!(sea_query::UpdateStatement);
build_query_stmt!(sea_query::DeleteStatement);

macro_rules! build_schema_stmt {
    ($stmt: ty) => {
        impl MigrationStatementBuilder for $stmt {
            fn build(&self, db_backend: &MigrationDbBackend) -> (String, Vec<Value>) {
                let stmt = build_any_stmt!(self, db_backend);
                (stmt, Vec::new())
            }
        }
    };
}

build_schema_stmt!(sea_query::TableCreateStatement);
build_schema_stmt!(sea_query::TableDropStatement);
build_schema_stmt!(sea_query::TableAlterStatement);
build_schema_stmt!(sea_query::TableRenameStatement);
build_schema_stmt!(sea_query::TableTruncateStatement);
build_schema_stmt!(sea_query::IndexCreateStatement);
build_schema_stmt!(sea_query::IndexDropStatement);
build_schema_stmt!(sea_query::ForeignKeyCreateStatement);
build_schema_stmt!(sea_query::ForeignKeyDropStatement);

macro_rules! build_type_stmt {
    ($stmt: ty) => {
        impl MigrationStatementBuilder for $stmt {
            fn build(&self, db_backend: &MigrationDbBackend) -> (String, Vec<Value>) {
                build_postgres_stmt!(self, db_backend)
            }
        }
    };
}

build_type_stmt!(sea_query::extension::postgres::TypeAlterStatement);
build_type_stmt!(sea_query::extension::postgres::TypeCreateStatement);
build_type_stmt!(sea_query::extension::postgres::TypeDropStatement);

impl MigrationStatementBuilder for String {
    fn build(&self, _: &MigrationDbBackend) -> (String, Vec<Value>) {
        (self.to_string(), Vec::new())
    }
}
