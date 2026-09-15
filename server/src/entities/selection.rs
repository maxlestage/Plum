use sea_orm::entity::prelude::*;

/// Une personne proposée un jour donné.
///
/// Voir `m20260101_000008_create_selections` pour ce que la table garantit.
/// En deux mots : la sélection du jour est stable parce qu'elle est écrite, et
/// elle est plafonnée parce qu'elle n'est tirée qu'une fois.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "selections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub viewer_id: Uuid,
    pub target_id: Uuid,
    pub served_on: Date,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
