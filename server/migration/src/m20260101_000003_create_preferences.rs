use sea_orm_migration::prelude::*;

use crate::m20260101_000001_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Preferences::Table)
                    .if_not_exists()
                    // Same identifier as the account, like the profile: one set
                    // of filters per member, and nothing to join on.
                    .col(
                        ColumnDef::new(Preferences::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Preferences::InterestedIn)
                            .string_len(16)
                            .not_null()
                            .default("everyone"),
                    )
                    // The defaults match the client's `DiscoveryPreferences
                    // .default`, so a member who never opens the settings sees
                    // the same deck the app promises them.
                    .col(
                        ColumnDef::new(Preferences::MinAge)
                            .integer()
                            .not_null()
                            .default(18),
                    )
                    .col(
                        ColumnDef::new(Preferences::MaxAge)
                            .integer()
                            .not_null()
                            .default(45),
                    )
                    .col(
                        ColumnDef::new(Preferences::MaxDistanceKm)
                            .integer()
                            .not_null()
                            .default(50),
                    )
                    .col(
                        ColumnDef::new(Preferences::ShowMeOnPlum)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Preferences::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Preferences::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preferences_user")
                            .from(Preferences::Table, Preferences::Id)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // The age range is meaningless inverted, and Plum is 18+ whatever the
        // client sends. The handler clamps both, and this makes the rule the
        // database's too: a future endpoint that forgets cannot write a row
        // the deck would then have to defend itself against.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE preferences
                   ADD CONSTRAINT preferences_age_range_is_legal
                   CHECK (min_age >= 18 AND max_age >= min_age AND max_age <= 120)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE preferences
                   ADD CONSTRAINT preferences_distance_is_positive
                   CHECK (max_distance_km >= 1)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Preferences::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Preferences {
    Table,
    Id,
    InterestedIn,
    MinAge,
    MaxAge,
    MaxDistanceKm,
    ShowMeOnPlum,
    CreatedAt,
    UpdatedAt,
}
