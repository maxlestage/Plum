use sea_orm::entity::prelude::*;

/// La trace d'une décision de modération.
///
/// Ajoutée, jamais modifiée : lever une suspension écrit une ligne de plus,
/// elle n'efface pas celle qui l'a posée. L'état courant vit sur
/// `users.suspended_at` et répond à « est-ce fermé maintenant ». Cette table
/// répond à « qu'est-ce qui s'est passé », qui n'est pas la même question et
/// qui est celle qu'on pose quand une décision est contestée.
///
/// `subject_id` est nullable, comme les deux bouts d'un signalement : la trace
/// survit au compte qu'elle nomme. Sans ça, supprimer son compte effacerait
/// son passif, et il suffirait de partir et de revenir.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "moderation_actions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub subject_id: Option<Uuid>,
    pub report_id: Option<Uuid>,
    pub action: String,
    pub reason: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
