use sea_orm::entity::prelude::*;

/// A match belongs to a pair, not to a direction: one row, with the smaller
/// identifier first. `match` is a keyword, hence the file name.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "matches")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub lower_id: Uuid,
    pub upper_id: Uuid,
    pub matched_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// The other person, seen from `viewer`.
    pub fn other(&self, viewer: Uuid) -> Uuid {
        if self.lower_id == viewer {
            self.upper_id
        } else {
            self.lower_id
        }
    }
}

/// Orders a pair the way the table stores it. Every write goes through this:
/// a row inserted the other way round would slip past the unique key and
/// create the duplicate match it exists to prevent.
pub fn ordered(a: Uuid, b: Uuid) -> (Uuid, Uuid) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
