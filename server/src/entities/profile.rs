use sea_orm::entity::prelude::*;

// No `Eq`: the coordinates are floating point, which has no total equality.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "profiles")]
pub struct Model {
    /// Shares the account's identifier: one profile per account, and the
    /// client already treats the two as the same thing.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub display_name: String,
    pub birth_date: Date,
    pub gender: String,
    pub bio: String,
    pub city: String,
    pub interests: Vec<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub last_active_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::Id",
        to = "super::user::Column::Id",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
