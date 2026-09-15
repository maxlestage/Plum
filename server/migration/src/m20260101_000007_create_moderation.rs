use sea_orm_migration::prelude::*;

use crate::m20260101_000001_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// De quoi *agir* sur un signalement, et pas seulement le lire.
///
/// La file de modération existait déjà, en lecture seule. C'était la même
/// panne que le blocage décoratif, d'un cran plus haut : quelqu'un signale un
/// harcèlement, un modérateur le lit — et n'a aucun levier. Une file qu'on ne
/// peut que regarder ne modère rien.
///
/// Trois choses ici, et la séparation est délibérée :
///
/// - `users.suspended_at` : l'**état courant**, une seule colonne, parce que
///   c'est ce que lisent la connexion et le deck à chaque requête.
/// - `moderation_actions` : la **trace**, jamais lue sur un chemin chaud.
///   Une décision qui touche le compte de quelqu'un doit pouvoir être relue,
///   y compris quand elle a été annulée depuis. L'état courant seul ne dit
///   pas qu'il y a eu trois suspensions avant.
/// - `reports.resolved_at` : ce qui fait qu'une file se **vide**. Sans ça,
///   le même signalement remonte à chaque ouverture et personne ne sait ce
///   qui a déjà été jugé.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Nullable, et c'est le fond de l'affaire : l'absence de date *est*
        // l'état normal. Un booléen `suspended` dirait la même chose en
        // perdant le quand, qui est la première question qu'on pose devant
        // une suspension contestée.
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Users::SuspendedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Reports::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Reports::ResolvedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .add_column_if_not_exists(ColumnDef::new(Reports::Resolution).text().null())
                    .to_owned(),
            )
            .await?;

        // La file ouverte se lit du plus récent au plus ancien, et c'est la
        // requête qu'on fait cent fois. L'index la couvre ; les signalements
        // déjà jugés n'y sont pas, parce qu'ils ne sont jamais ce qu'on
        // cherche.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_reports_ouverts")
                    .table(Reports::Table)
                    .col(Reports::CreatedAt)
                    .col(Reports::Id)
                    .and_where(Expr::col(Reports::ResolvedAt).is_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ModerationActions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModerationActions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    // Nullable, comme les deux bouts d'un signalement : la
                    // trace survit au compte qu'elle nomme. Quelqu'un qui
                    // supprime son compte efface ses données, pas le fait
                    // qu'une décision a été prise — sans quoi il suffirait de
                    // partir et revenir pour effacer son passif.
                    .col(ColumnDef::new(ModerationActions::SubjectId).uuid().null())
                    .col(ColumnDef::new(ModerationActions::ReportId).uuid().null())
                    .col(ColumnDef::new(ModerationActions::Action).text().not_null())
                    .col(ColumnDef::new(ModerationActions::Reason).text().not_null())
                    .col(
                        ColumnDef::new(ModerationActions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_moderation_subject")
                            .from(ModerationActions::Table, ModerationActions::SubjectId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_moderation_report")
                            .from(ModerationActions::Table, ModerationActions::ReportId)
                            .to(Reports::Table, Reports::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // L'historique d'une personne se lit par personne, et du plus récent
        // au plus ancien : « est-ce la première fois ? » est la question qu'on
        // pose avant de suspendre.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_moderation_sujet")
                    .table(ModerationActions::Table)
                    .col(ModerationActions::SubjectId)
                    .col(ModerationActions::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ModerationActions::Table).to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_reports_ouverts")
                    .table(Reports::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Reports::Table)
                    .drop_column(Reports::ResolvedAt)
                    .drop_column(Reports::Resolution)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::SuspendedAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Reports {
    Table,
    Id,
    CreatedAt,
    ResolvedAt,
    Resolution,
}

#[derive(DeriveIden)]
enum ModerationActions {
    Table,
    Id,
    SubjectId,
    ReportId,
    Action,
    Reason,
    CreatedAt,
}
