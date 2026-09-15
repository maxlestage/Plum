pub use sea_orm_migration::prelude::*;

pub mod m20260101_000001_create_users;
mod m20260101_000002_create_profiles;
mod m20260101_000003_create_preferences;
pub mod m20260101_000004_create_deck;
mod m20260101_000005_create_chat;
mod m20260101_000006_create_photos;
mod m20260101_000007_create_moderation;

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
            Box::new(m20260101_000006_create_photos::Migration),
            Box::new(m20260101_000007_create_moderation::Migration),
        ]
    }
}
