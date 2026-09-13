use sea_orm::entity::prelude::*;

/// Both sides are nullable: a report outlives the accounts it names, because
/// it is the record of why a decision was taken.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "reports")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub reporter_id: Option<Uuid>,
    pub reported_id: Option<Uuid>,
    pub reason: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
