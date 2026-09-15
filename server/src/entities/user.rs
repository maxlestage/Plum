use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub email: String,
    pub password_hash: String,
    pub profile_completed: bool,
    /// Quand la modération a fermé ce compte, et `None` le reste du temps.
    ///
    /// L'absence de date est l'état normal, donc la colonne est nullable
    /// plutôt qu'un booléen : la première question devant une suspension
    /// contestée est « depuis quand », et un booléen ne la garde pas.
    pub suspended_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::refresh_token::Entity")]
    RefreshToken,
    #[sea_orm(
        belongs_to = "super::profile::Entity",
        from = "Column::Id",
        to = "super::profile::Column::Id"
    )]
    Profile,
}

impl Related<super::refresh_token::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RefreshToken.def()
    }
}

impl Related<super::profile::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Profile.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
