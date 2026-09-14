use sea_orm_migration::prelude::*;

use crate::m20260101_000001_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Les octets vivent dans la base, pas dans un stockage objet.
        //
        // Ce n'est pas l'endroit habituel, et c'est assumé : un stockage objet
        // est un service payant de plus, et l'application n'est pas lancée.
        // Le plafond est connu et **borné**, pas estimé : le plan Postgres
        // tient un gigaoctet et une photo stockée ne peut pas dépasser 200 Kio
        // — l'encodeur baisse la qualité jusqu'à y tenir, et deux tests le
        // vérifient, dont un sur le pire cas qui comprime le plus mal. Six
        // photos par profil font donc de l'ordre du millier de profils même
        // dans le pire cas. Le jour où l'on s'en approche, seule cette table
        // change : l'adresse publique reste `/photos/{id}`, et le client ne
        // verra rien.
        //
        // `bytea` plutôt qu'un chemin sur disque : le système de fichiers d'un
        // dyno est effacé à chaque redémarrage, donc un fichier écrit là
        // disparaît en même temps que la promesse de l'afficher.
        manager
            .create_table(
                Table::create()
                    .table(Photos::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Photos::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Photos::ProfileId).uuid().not_null())
                    // 0 est la couverture : la seule photo qui décide si
                    // quelqu'un lira le reste.
                    .col(ColumnDef::new(Photos::Position).integer().not_null())
                    .col(ColumnDef::new(Photos::ContentType).text().not_null())
                    .col(ColumnDef::new(Photos::Width).integer().not_null())
                    .col(ColumnDef::new(Photos::Height).integer().not_null())
                    .col(ColumnDef::new(Photos::Bytes).binary().not_null())
                    .col(
                        ColumnDef::new(Photos::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_photos_profile")
                            .from(Photos::Table, Photos::ProfileId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Le profil lit ses photos dans l'ordre, et rien d'autre ne les lit
        // par propriétaire.
        manager
            .create_index(
                Index::create()
                    .name("idx_photos_profile")
                    .table(Photos::Table)
                    .col(Photos::ProfileId)
                    .col(Photos::Position)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Photos::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Photos {
    Table,
    Id,
    ProfileId,
    Position,
    ContentType,
    Width,
    Height,
    Bytes,
    CreatedAt,
}
