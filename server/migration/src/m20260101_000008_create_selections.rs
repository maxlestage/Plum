use sea_orm_migration::prelude::*;

use crate::m20260101_000001_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// La sélection du jour : une poignée de profils, et c'est tout.
///
/// Elle remplace le deck, qui n'avait pas de fond. Un deck sans fin demande un
/// geste sans fin — c'est ce qui rend le balayage nécessaire, et c'est
/// précisément ce dont on ne veut plus. Une poignée de profils tient sur un
/// écran et se décide au bouton.
///
/// La table sert deux choses à la fois, et c'est pour ça qu'elle existe plutôt
/// que d'être calculée :
///
/// - **la stabilité.** Sans trace de ce qui a été servi, rouvrir l'application
///   retirerait la requête et rendrait d'autres personnes. La sélection du
///   jour doit être la même le matin et le soir.
/// - **le plafond.** Trois par jour n'est pas un quota posé à côté : c'est la
///   conséquence du tirage unique. Il n'y a rien à compter.
///
/// Ce qui n'est **pas** ici : la contrainte d'unicité ne porte pas seulement
/// sur la paire mais sur la paire *et le jour*. Quelqu'un qu'on vous a proposé
/// et sur qui vous n'avez rien décidé retourne dans le tirage. Le brûler
/// coûterait des rencontres à une application qui n'en a pas encore beaucoup,
/// et « ne pas avoir ouvert l'application ce jour-là » n'est pas une décision.
/// Ce qui sort définitivement du tirage, c'est ce qui est tranché — et ça se
/// lit dans `swipes`, pas ici.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Selections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Selections::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Selections::ViewerId).uuid().not_null())
                    .col(ColumnDef::new(Selections::TargetId).uuid().not_null())
                    // Une date, pas un horodatage : « le jour de » est la
                    // question posée, et la poser sur un `timestamptz`
                    // obligerait à choisir un fuseau à chaque lecture.
                    .col(ColumnDef::new(Selections::ServedOn).date().not_null())
                    .col(
                        ColumnDef::new(Selections::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_selections_viewer")
                            .from(Selections::Table, Selections::ViewerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_selections_target")
                            .from(Selections::Table, Selections::TargetId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // La sélection du jour se lit par (personne, jour), et c'est la seule
        // requête que cette table sert.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_selections_du_jour")
                    .table(Selections::Table)
                    .col(Selections::ViewerId)
                    .col(Selections::ServedOn)
                    .to_owned(),
            )
            .await?;

        // Deux fois la même personne dans la même sélection serait un tirage
        // raté servi tel quel.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_selections_unique")
                    .table(Selections::Table)
                    .col(Selections::ViewerId)
                    .col(Selections::TargetId)
                    .col(Selections::ServedOn)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Selections::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Selections {
    Table,
    Id,
    ViewerId,
    TargetId,
    ServedOn,
    CreatedAt,
}
