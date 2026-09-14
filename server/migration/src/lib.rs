pub use sea_orm_migration::prelude::*;

pub mod m20260101_000001_create_users;
mod m20260101_000002_create_profiles;
mod m20260101_000003_create_preferences;
pub mod m20260101_000004_create_deck;
mod m20260101_000005_create_chat;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260101_000001_create_users::Migration),
            Box::new(m20260101_000002_create_profiles::Migration),
            Box::new(m20260101_000003_create_preferences::Migration),
            Box::new(m20260101_000004_create_deck::Migration),
            Box::new(m20260101_000005_create_chat::Migration),
        ]
    }
}
