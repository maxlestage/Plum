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
                    .table(Profiles::Table)
                    .if_not_exists()
                    // The profile shares the account's identifier: one profile
                    // per account, and the client already treats them as the
                    // same id.
                    .col(ColumnDef::new(Profiles::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Profiles::DisplayName)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Profiles::BirthDate).date().not_null())
                    .col(ColumnDef::new(Profiles::Gender).string_len(32).not_null())
                    .col(ColumnDef::new(Profiles::Bio).text().not_null().default(""))
                    .col(
                        ColumnDef::new(Profiles::City)
                            .string_len(120)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(Profiles::Interests)
                            .array(ColumnType::Text)
                            .not_null()
                            .default(Expr::cust("ARRAY[]::text[]")),
                    )
                    .col(ColumnDef::new(Profiles::Latitude).double().null())
                    .col(ColumnDef::new(Profiles::Longitude).double().null())
                    .col(
                        ColumnDef::new(Profiles::LastActiveAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Profiles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Profiles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_profiles_user")
                            .from(Profiles::Table, Profiles::Id)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Profiles::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Profiles {
    Table,
    Id,
    DisplayName,
    BirthDate,
    Gender,
    Bio,
    City,
    Interests,
    Latitude,
    Longitude,
    LastActiveAt,
    CreatedAt,
    UpdatedAt,
}
