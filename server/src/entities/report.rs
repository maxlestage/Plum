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
    /// Quand quelqu'un l'a jugé. Tant que c'est `None`, il est dans la file.
    pub resolved_at: Option<DateTimeWithTimeZone>,
    /// Ce qui a été décidé, en clair. Le libellé est celui de l'API — voir
    /// `Resolution` — et il est relu par un humain, pas par du code.
    pub resolution: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
