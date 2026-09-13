use sea_orm_migration::prelude::*;

use crate::m20260101_000001_create_users::Users;
use crate::m20260101_000004_create_deck::Matches;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Une conversation par match, et pas davantage : la clé unique évite
        // que deux ouvertures simultanées n'en créent deux, ce qui couperait
        // la discussion en deux moitiés invisibles l'une à l'autre.
        manager
            .create_table(
                Table::create()
                    .table(Conversations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Conversations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Conversations::MatchId)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Conversations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // Ce que la liste trie : la date du dernier message, ou
                    // celle de l'ouverture tant que personne n'a rien dit.
                    .col(
                        ColumnDef::new(Conversations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_conversations_match")
                            .from(Conversations::Table, Conversations::MatchId)
                            .to(Matches::Table, Matches::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Messages::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Messages::ConversationId).uuid().not_null())
                    .col(ColumnDef::new(Messages::SenderId).uuid().not_null())
                    // Fourni par le client : une connexion qui lâche entre
                    // l'envoi et la réponse fait réessayer, et sans cette clé
                    // le message partirait deux fois.
                    .col(ColumnDef::new(Messages::ClientId).uuid().not_null())
                    .col(ColumnDef::new(Messages::Body).text().not_null())
                    .col(
                        ColumnDef::new(Messages::SentAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Messages::ReadAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_messages_conversation")
                            .from(Messages::Table, Messages::ConversationId)
                            .to(Conversations::Table, Conversations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_messages_sender")
                            .from(Messages::Table, Messages::SenderId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_messages_client")
                    .table(Messages::Table)
                    .col(Messages::ConversationId)
                    .col(Messages::ClientId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // La lecture d'une conversation remonte le fil du plus récent au plus
        // ancien ; c'est l'ordre que cet index sert.
        manager
            .create_index(
                Index::create()
                    .name("idx_messages_thread")
                    .table(Messages::Table)
                    .col(Messages::ConversationId)
                    .col((Messages::SentAt, IndexOrder::Desc))
                    .col((Messages::Id, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // Un message vide n'est pas un message, et les espaces sont coupés
        // avant l'insertion. La contrainte protège la prochaine route qui
        // écrira ici en l'oubliant.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE messages
                   ADD CONSTRAINT messages_body_is_not_empty
                   CHECK (length(btrim(body)) > 0)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Messages::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Conversations::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Conversations {
    Table,
    Id,
    MatchId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Messages {
    Table,
    Id,
    ConversationId,
    SenderId,
    ClientId,
    Body,
    SentAt,
    ReadAt,
}
