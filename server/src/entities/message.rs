use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    /// Fourni par le client, unique dans la conversation : un renvoi après une
    /// connexion coupée retrouve le message déjà écrit au lieu d'en créer un
    /// second.
    pub client_id: Uuid,
    pub body: String,
    pub sent_at: DateTimeWithTimeZone,
    pub read_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
