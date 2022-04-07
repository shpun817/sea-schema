use super::MigrationQueryResult;
use sea_query::Iden;

#[derive(Iden)]
#[iden = "seaql_migrations"]
pub struct Table;

#[derive(Iden)]
pub enum Column {
    Version,
    AppliedAt,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub version: String,
    pub applied_at: i64,
}

impl Model {
    pub fn try_from_query_result<R>(res: R) -> Result<Self, R::Error>
    where
        R: MigrationQueryResult,
    {
        Ok(Self {
            version: res.try_get_string("version")?,
            applied_at: res.try_get_i64("applied_at")?,
        })
    }
}
