use sea_orm_migration::prelude::*;

use crate::m20260101_000001_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // A verdict, once, per pair. The unique key is what stops a card
        // coming back: the deck excludes anyone this account has already
        // judged, and without the constraint a double tap would write two
        // rows and the rewind would undo only one of them.
        manager
            .create_table(
                Table::create()
                    .table(Swipes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Swipes::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Swipes::ViewerId).uuid().not_null())
                    .col(ColumnDef::new(Swipes::TargetId).uuid().not_null())
                    .col(ColumnDef::new(Swipes::Decision).string_len(16).not_null())
                    .col(
                        ColumnDef::new(Swipes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_swipes_viewer")
                            .from(Swipes::Table, Swipes::ViewerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_swipes_target")
                            .from(Swipes::Table, Swipes::TargetId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_swipes_viewer_target")
                    .table(Swipes::Table)
                    .col(Swipes::ViewerId)
                    .col(Swipes::TargetId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Rewind reads the most recent pass; the deck reads every verdict of
        // one viewer. Both want this order.
        manager
            .create_index(
                Index::create()
                    .name("idx_swipes_viewer_recent")
                    .table(Swipes::Table)
                    .col(Swipes::ViewerId)
                    .col((Swipes::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // A match belongs to a pair, not to a direction. Storing it once with
        // the smaller id first — and a unique key on the ordered pair — makes
        // a duplicate impossible even if both people like each other at the
        // same instant.
        manager
            .create_table(
                Table::create()
                    .table(Matches::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Matches::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Matches::LowerId).uuid().not_null())
                    .col(ColumnDef::new(Matches::UpperId).uuid().not_null())
                    .col(
                        ColumnDef::new(Matches::MatchedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_matches_lower")
                            .from(Matches::Table, Matches::LowerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_matches_upper")
                            .from(Matches::Table, Matches::UpperId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_matches_pair")
                    .table(Matches::Table)
                    .col(Matches::LowerId)
                    .col(Matches::UpperId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // The ordering is an invariant, not a convention: a row written the
        // other way round would escape the unique key above and produce the
        // duplicate match it exists to prevent.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE matches
                   ADD CONSTRAINT matches_pair_is_ordered
                   CHECK (lower_id < upper_id)",
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Blocks::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Blocks::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Blocks::BlockerId).uuid().not_null())
                    .col(ColumnDef::new(Blocks::BlockedId).uuid().not_null())
                    .col(
                        ColumnDef::new(Blocks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blocks_blocker")
                            .from(Blocks::Table, Blocks::BlockerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blocks_blocked")
                            .from(Blocks::Table, Blocks::BlockedId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blocks_pair")
                    .table(Blocks::Table)
                    .col(Blocks::BlockerId)
                    .col(Blocks::BlockedId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Blocking hides in both directions, so the deck asks "is there a
        // block either way", which reads this index.
        manager
            .create_index(
                Index::create()
                    .name("idx_blocks_blocked")
                    .table(Blocks::Table)
                    .col(Blocks::BlockedId)
                    .to_owned(),
            )
            .await?;

        // Reports are kept even after the reported account is gone: they are
        // the record of why a decision was made. Hence no cascade on the
        // reported side — the row survives with a dangling reference rather
        // than vanishing with the evidence.
        manager
            .create_table(
                Table::create()
                    .table(Reports::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Reports::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Reports::ReporterId).uuid().null())
                    .col(ColumnDef::new(Reports::ReportedId).uuid().null())
                    .col(ColumnDef::new(Reports::Reason).text().not_null())
                    .col(
                        ColumnDef::new(Reports::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_reports_reporter")
                            .from(Reports::Table, Reports::ReporterId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_reports_reported")
                            .from(Reports::Table, Reports::ReportedId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Four `DeriveIden` enums are four types, so no loop. Dropped in
        // reverse order of creation, though nothing here references anything
        // else.
        manager
            .drop_table(Table::drop().table(Reports::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Blocks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Matches::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Swipes::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Swipes {
    Table,
    Id,
    ViewerId,
    TargetId,
    Decision,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum Matches {
    Table,
    Id,
    LowerId,
    UpperId,
    MatchedAt,
}

#[derive(DeriveIden)]
enum Blocks {
    Table,
    Id,
    BlockerId,
    BlockedId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Reports {
    Table,
    Id,
    ReporterId,
    ReportedId,
    Reason,
    CreatedAt,
}
