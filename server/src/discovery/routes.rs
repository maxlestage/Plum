use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{NaiveTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use super::deck_query::{among as query_among, deck as query_deck, Candidate};
use super::types::*;
use crate::auth::routes::authenticate;
use crate::auth::types::Gender;
use crate::entities::{
    block, conversation, match_pair, message, profile, report, selection, swipe,
};
use crate::error::{ApiError, ApiResult};
use crate::live::routes::announce_match;
use crate::profile::types::ProfileResponse;
use crate::state::AppState;

/// Ce qu'on peut écrire dans un premier message. Le même plafond que dans une
/// conversation ouverte : c'est le même objet.
const MAX_BODY: usize = 2_000;
const MAX_REASON: usize = 500;

/// Combien de jours une personne déjà proposée attend avant de pouvoir
/// revenir dans un tirage.
///
/// Sans cette mise à l'écart, quelqu'un qui n'ouvre l'application que pour
/// regarder revoit les mêmes trois visages tous les jours : le tirage prend
/// les plus proches, et ne rien décider ne change rien au classement. Mesuré,
/// pas supposé — et l'écran, lui, promettait « trois autres profils demain ».
///
/// Une semaine, et pas « pour toujours » : ne pas avoir tranché n'est pas une
/// décision, et brûler définitivement quelqu'un qu'on n'a fait qu'entrevoir
/// coûterait des rencontres à une application qui n'en a pas encore beaucoup.
const JOURS_AVANT_DE_REVENIR: i64 = 7;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/discovery/selection", get(selection_of_the_day))
        // Écrire et laisser passer sont les deux seules issues, et elles
        // vivent sur le profil plutôt que sous `/discovery` : ce qu'on fait,
        // c'est écrire *à quelqu'un*, pas rendre un verdict à un moteur.
        .route("/profiles/{id}/write", post(write_first))
        .route("/profiles/{id}/pass", post(pass_profile))
        .route("/profiles/{id}/report", post(report_profile))
        .route(
            "/profiles/{id}/block",
            post(block_profile).delete(unblock_profile),
        )
        .route("/me/blocks", get(list_blocks))
}

fn candidate_into_profile(candidate: Candidate) -> ProfileResponse {
    ProfileResponse {
        id: candidate.id,
        display_name: candidate.display_name,
        birth_date: candidate.birth_date.and_time(NaiveTime::MIN).and_utc(),
        gender: Gender::parse(&candidate.gender).unwrap_or(Gender::Other),
        bio: candidate.bio,
        city: candidate.city,
        photos: Vec::new(),
        interests: candidate.interests,
        distance_km: candidate.distance_km,
        last_active_at: candidate.last_active_at.map(Into::into),
    }
}

/// La sélection du jour : une poignée de profils, tirés une fois, stables
/// jusqu'à minuit.
///
/// Ce qu'elle remplace, et pourquoi. Le deck n'avait pas de fond : il se
/// paginait, et il fallait un geste par carte pour avancer. Un geste qu'on
/// répète cent fois doit être minuscule — d'où le balayage, et d'où le fait
/// qu'on décide de quelqu'un en un quart de seconde. Deux à cinq profils
/// tiennent sur un écran et se lisent.
///
/// Le tirage est écrit en base, pas recalculé. Sans ça, rouvrir l'application
/// rendrait trois autres personnes, et « la sélection du jour » ne voudrait
/// rien dire. C'est aussi ce qui plafonne : il n'y a pas de quota à compter,
/// on tire une fois.
async fn selection_of_the_day(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<SelectionResponse>> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;
    let today = Utc::now().date_naive();
    let taille = taille_du_jour(
        state.config.daily_selection_min,
        state.config.daily_selection_max,
        viewer,
        today,
    );

    let mut deja = selection::Entity::find()
        .filter(selection::Column::ViewerId.eq(viewer))
        .filter(selection::Column::ServedOn.eq(today))
        .order_by_asc(selection::Column::CreatedAt)
        .all(&state.db)
        .await?;

    // Rien pour aujourd'hui : on tire. `query_deck` applique déjà tous les
    // écarts — déjà tranché, bloqué d'un côté ou de l'autre, retiré, suspendu,
    // hors des critères — donc il n'y a rien à refiltrer ici.
    if deja.is_empty() {
        for candidat in draw(&state, viewer, today, taille).await? {
            selection::ActiveModel {
                id: Set(Uuid::new_v4()),
                viewer_id: Set(viewer),
                target_id: Set(candidat),
                served_on: Set(today),
                created_at: Set(Utc::now().into()),
            }
            .insert(&state.db)
            .await?;
        }
        deja = selection::Entity::find()
            .filter(selection::Column::ViewerId.eq(viewer))
            .filter(selection::Column::ServedOn.eq(today))
            .order_by_asc(selection::Column::CreatedAt)
            .all(&state.db)
            .await?;
    }

    // Une personne tirée ce matin a pu, depuis, être tranchée, se retirer, ou
    // être suspendue. La sélection est stable, pas figée : ce qui n'a plus
    // lieu d'être montré ne l'est plus, et rien ne vient le remplacer — la
    // sélection du jour a été tirée.
    let vises: Vec<Uuid> = deja.iter().map(|ligne| ligne.target_id).collect();
    let encore = selection_still_showable(&state, viewer, &vises).await?;

    let mut items: Vec<ProfileResponse> = encore.into_iter().map(candidate_into_profile).collect();
    crate::photos::routes::attach(&state, &mut items.iter_mut().collect::<Vec<_>>()).await?;

    Ok(Json(SelectionResponse {
        items,
        refreshes_at: (today + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .expect("minuit existe")
            .and_utc(),
        // Ce qui a vraiment été tiré, pas ce qu'on visait. L'écran s'en sert
        // pour dire « il en reste deux sur quatre » ; annoncer la cible ferait
        // mentir cette phrase les jours où le voisinage n'avait pas de quoi
        // remplir la sélection — « il en reste deux sur quatre » avec deux
        // profils à l'écran et rien qui vienne.
        size: deja.len() as u32,
    }))
}

/// Combien de profils aujourd'hui, pour cette personne.
///
/// Un compte fixe se transforme en habitude : on sait ce qu'on va trouver
/// avant d'ouvrir, on l'expédie, et l'application redevient la pile qu'elle ne
/// voulait pas être. Un nombre qui change se regarde.
///
/// Tiré d'un brassage de l'identifiant et du jour plutôt que du hasard, pour
/// une raison concrète : le tirage s'écrit en base ligne par ligne, et une
/// écriture coupée au milieu laisse une sélection partielle. Redemander la
/// même journée doit retomber sur la même cible, sinon le rattrapage vise un
/// autre nombre que celui qu'il rattrape. C'est aussi ce qui permet de
/// l'éprouver : une suite peut faire défiler dix jours et lire la suite des
/// comptes, ce qu'un `rand` rendrait invérifiable.
///
/// Imprévisible pour qui le vit, reproductible pour qui le vérifie — les deux
/// comptent, et ce n'est pas la même exigence que celle d'un générateur
/// cryptographique : personne ne gagne rien à deviner qu'il aura quatre
/// profils demain.
///
/// Publique pour les suites d'intégration : elles ne peuvent pas avancer
/// l'horloge du serveur — `advance_one_day` recule les lignes déjà servies,
/// ce qui ne change pas la date que cette fonction reçoit — donc elles
/// vérifient que le tirage du jour vaut bien ce qu'elle annonce pour
/// aujourd'hui, et le module de tests ci-dessous se charge de la variation
/// d'un jour à l'autre.
pub fn taille_du_jour(min: u32, max: u32, viewer: Uuid, jour: chrono::NaiveDate) -> u32 {
    debug_assert!(min <= max, "la configuration redresse déjà la fourchette");
    let etendue = u64::from(max.saturating_sub(min)) + 1;
    min + (brassage(viewer, jour) % etendue) as u32
}

/// FNV-1a, écrit ici plutôt qu'emprunté à `DefaultHasher`.
///
/// `DefaultHasher` ne promet rien sur la stabilité de ses valeurs d'une
/// version de Rust à l'autre : une mise à jour du compilateur redistribuerait
/// les comptes de tout le monde, et les suites qui lisent une séquence
/// précise tomberaient sans que rien du produit n'ait changé.
fn brassage(viewer: Uuid, jour: chrono::NaiveDate) -> u64 {
    use chrono::Datelike;
    const BASE: u64 = 0xcbf2_9ce4_8422_2325;
    const PREMIER: u64 = 0x0000_0100_0000_01b3;

    let mut empreinte = BASE;
    let mut avaler = |octet: u8| {
        empreinte ^= u64::from(octet);
        empreinte = empreinte.wrapping_mul(PREMIER);
    };
    for octet in viewer.as_bytes() {
        avaler(*octet);
    }
    for octet in jour.num_days_from_ce().to_le_bytes() {
        avaler(octet);
    }
    empreinte
}

/// Le tirage du jour : qui, et dans quel ordre.
///
/// Deux règles, dans cet ordre.
///
/// 1. **Les personnes qu'on n'a pas vues récemment d'abord**, les plus proches
///    en tête. C'est ce qui rend vrai le « trois autres profils demain » que
///    l'écran affiche : sans cette règle, le tirage reprend les plus proches,
///    et quelqu'un qui ne décide de rien revoit les trois mêmes visages
///    indéfiniment. Vérifié en reculant la date d'un jour : c'était exactement
///    ce qui se produisait.
///
/// 2. **Puis, si ça ne suffit pas, les autres**, du plus anciennement proposé
///    au plus récent. Sur une base de profils petite, la première règle seule
///    donnerait un écran à moitié vide alors qu'il y a des gens à montrer —
///    et un écran vide est un mensonge d'un autre genre.
async fn draw(
    state: &AppState,
    viewer: Uuid,
    today: chrono::NaiveDate,
    taille: u32,
) -> ApiResult<Vec<Uuid>> {
    // Large exprès : il faut de quoi écarter les déjà-vus et tomber quand même
    // sur le compte du jour.
    let candidats = query_deck(&state.db, viewer, taille.saturating_mul(20).max(50), None).await?;

    // Quand chacun a été proposé pour la dernière fois. Absent de la table
    // veut dire jamais.
    let depuis = today - chrono::Duration::days(JOURS_AVANT_DE_REVENIR);
    let recents: std::collections::HashSet<Uuid> = selection::Entity::find()
        .filter(selection::Column::ViewerId.eq(viewer))
        .filter(selection::Column::ServedOn.gt(depuis))
        .all(&state.db)
        .await?
        .into_iter()
        .map(|ligne| ligne.target_id)
        .collect();

    let (frais, revus): (Vec<_>, Vec<_>) = candidats
        .into_iter()
        .map(|candidat| candidat.id)
        .partition(|id| !recents.contains(id));

    Ok(frais
        .into_iter()
        .chain(revus)
        .take(taille as usize)
        .collect())
}

/// Les profils d'une sélection déjà tirée, revus au présent.
///
/// Le même tirage, filtré sur les personnes déjà choisies. Ça coûte une
/// requête de plus que de relire les profils directement, et ça évite de
/// montrer quelqu'un qui vous a bloqué depuis ce matin, s'est retiré, ou a été
/// suspendu : la sélection est stable, pas figée.
///
/// Ce qui disparaît n'est **pas** remplacé. La sélection du jour a été tirée ;
/// la remplir à nouveau ferait du départ de quelqu'un une occasion d'en voir
/// un de plus.
async fn selection_still_showable(
    state: &AppState,
    viewer: Uuid,
    vises: &[Uuid],
) -> ApiResult<Vec<Candidate>> {
    let encore = query_among(&state.db, viewer, vises).await?;
    let mut par_id: std::collections::HashMap<Uuid, Candidate> = encore
        .into_iter()
        .map(|candidat| (candidat.id, candidat))
        .collect();

    // L'ordre de la sélection, pas celui de la distance : ces trois-là sont
    // posées, et les voir changer de place d'une ouverture à l'autre donnerait
    // l'impression qu'elles ont changé.
    Ok(vises
        .iter()
        .filter_map(|vise| par_id.remove(vise))
        .collect())
}

/// Laisser passer. C'est une décision, et elle est définitive.
async fn pass_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<()>> {
    let claims = authenticate(&state, &headers)?;
    record_verdict(&state, claims.sub, id, Verdict::Passed).await?;
    Ok(Json(()))
}

/// Écrire, et c'est tout ce qu'il y a à faire d'un profil qui plaît.
///
/// Il n'y a plus de « j'aime » qui attend sa réciproque, donc plus d'écran
/// « c'est un match ». Une conversation existe parce que quelqu'un a écrit
/// quelque chose ; l'autre répond ou ne répond pas, et ne pas répondre est une
/// réponse qui n'a besoin d'aucune interface.
///
/// Le `match_pair` reste, et il ne veut plus dire « double oui » : il veut dire
/// « ces deux-là ont un fil ». Le reste du serveur — la liste, le socket, le
/// blocage qui referme — s'appuie dessus et n'a pas à savoir d'où il vient.
async fn write_first(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<WriteRequest>,
) -> ApiResult<Json<crate::chat::types::MessageResponse>> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;

    let body = request.body.trim();
    if body.is_empty() {
        return Err(ApiError::BadRequest(
            "Un message vide n'en est pas un.".into(),
        ));
    }
    if body.chars().count() > MAX_BODY {
        return Err(ApiError::BadRequest("Ce message est trop long.".into()));
    }

    // On n'écrit qu'à quelqu'un de sa sélection du jour. C'est ça, la limite —
    // pas un quota posé à côté. Sans elle, l'adresse accepterait n'importe
    // quel identifiant et rendrait l'envoi en masse trivial.
    let today = Utc::now().date_naive();
    let propose = selection::Entity::find()
        .filter(selection::Column::ViewerId.eq(viewer))
        .filter(selection::Column::TargetId.eq(id))
        .filter(selection::Column::ServedOn.eq(today))
        .one(&state.db)
        .await?;
    if propose.is_none() {
        return Err(ApiError::NotFound);
    }

    record_verdict(&state, viewer, id, Verdict::Written).await?;

    let now = Utc::now();
    let (lower, upper) = match_pair::ordered(viewer, id);
    let corps = body.to_owned();

    // Le fil, la conversation et le premier message partent ensemble. Un fil
    // créé sans son message laisserait une conversation vide chez quelqu'un
    // qui n'a jamais rien reçu, et aucune requête plus tard ne le réparerait.
    let ecrit = state
        .db
        .transaction::<_, message::Model, sea_orm::DbErr>(move |txn| {
            Box::pin(async move {
                let pair = match_pair::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    lower_id: Set(lower),
                    upper_id: Set(upper),
                    matched_at: Set(now.into()),
                }
                .insert(txn)
                .await?;

                let fil = conversation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    match_id: Set(pair.id),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;

                message::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    conversation_id: Set(fil.id),
                    sender_id: Set(viewer),
                    client_id: Set(Uuid::new_v4()),
                    body: Set(corps),
                    sent_at: Set(now.into()),
                    read_at: Set(None),
                }
                .insert(txn)
                .await
            })
        })
        .await
        .map_err(|erreur| match erreur {
            sea_orm::TransactionError::Transaction(inner) => ApiError::from(inner),
            sea_orm::TransactionError::Connection(inner) => ApiError::from(inner),
        })?;

    // L'autre l'apprend par le socket s'il est là, sinon en rouvrant. Le
    // message est en base avant d'être annoncé, donc rien ne se perd quand
    // personne n'écoute.
    if let Some(mien) = profile::Entity::find_by_id(viewer).one(&state.db).await? {
        let mut profil = ProfileResponse::own(mien);
        crate::photos::routes::attach_one(&state, &mut profil).await?;
        announce_match(
            &state,
            id,
            MatchResponse {
                id: ecrit.conversation_id,
                profile: profil,
                matched_at: now,
                conversation_id: Some(ecrit.conversation_id),
            },
        );
    }

    Ok(Json(ecrit.into()))
}

/// Écrit le verdict, et refuse ce qui ne se décide pas.
async fn record_verdict(
    state: &AppState,
    viewer: Uuid,
    target: Uuid,
    verdict: Verdict,
) -> ApiResult<()> {
    if viewer == target {
        return Err(ApiError::BadRequest(
            "On ne peut pas se juger soi-même.".into(),
        ));
    }

    // La cible doit exister, sans quoi une coquille s'écrirait comme un
    // verdict rendu sur personne.
    profile::Entity::find_by_id(target)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    swipe::ActiveModel {
        id: Set(Uuid::new_v4()),
        viewer_id: Set(viewer),
        target_id: Set(target),
        decision: Set(verdict.as_str().to_owned()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&state.db)
    .await
    .map_err(|erreur| match erreur {
        // La clé unique sur (viewer, target) qui parle : c'est déjà tranché.
        sea_orm::DbErr::Query(_) | sea_orm::DbErr::Exec(_) => {
            ApiError::Conflict("Ce profil a déjà été tranché.".into())
        }
        autre => ApiError::from(autre),
    })?;

    Ok(())
}

async fn report_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<ReportRequest>,
) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;

    let reason = request.reason.trim();
    if reason.is_empty() {
        return Err(ApiError::BadRequest("Le motif est obligatoire.".into()));
    }
    if reason.chars().count() > MAX_REASON {
        return Err(ApiError::BadRequest("Le motif est trop long.".into()));
    }

    report::ActiveModel {
        id: Set(Uuid::new_v4()),
        reporter_id: Set(Some(claims.sub)),
        reported_id: Set(Some(id)),
        reason: Set(reason.to_owned()),
        created_at: Set(Utc::now().into()),
        // Neuf, donc dans la file : c'est l'absence de date qui l'y met.
        resolved_at: Set(None),
        resolution: Set(None),
    }
    .insert(&state.db)
    .await?;

    Ok(())
}

/// Qui on a bloqué, avec de quoi les reconnaître.
///
/// Sans cette liste, le déblocage serait injoignable : on ne débloque pas
/// quelqu'un dont on ne se rappelle plus le nom, et il n'y a nulle part
/// ailleurs où le retrouver — le profil a disparu de partout, c'est tout
/// l'intérêt d'un blocage.
async fn list_blocks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<BlockedPerson>>> {
    let claims = authenticate(&state, &headers)?;

    let blocages = block::Entity::find()
        .filter(block::Column::BlockerId.eq(claims.sub))
        .order_by_desc(block::Column::CreatedAt)
        .all(&state.db)
        .await?;

    let vises: Vec<Uuid> = blocages.iter().map(|b| b.blocked_id).collect();
    let profils = profile::Entity::find()
        .filter(profile::Column::Id.is_in(vises))
        .all(&state.db)
        .await?;

    Ok(Json(
        blocages
            .into_iter()
            .map(|blocage| BlockedPerson {
                id: blocage.blocked_id,
                // Le compte a pu partir depuis : la ligne reste, le nom non.
                display_name: profils
                    .iter()
                    .find(|p| p.id == blocage.blocked_id)
                    .map(|p| p.display_name.clone()),
                blocked_at: blocage.created_at.into(),
            })
            .collect(),
    ))
}

/// Débloquer, et le faire vraiment.
///
/// Retirer la ligne de blocage ne suffit pas, et c'est mesuré : le tirage
/// écarte aussi les profils déjà tranchés, donc la personne resterait
/// invisible. La route rendrait 200, la liste se viderait, et rien ne
/// changerait à l'écran — une route décorative, exactement le défaut que ce
/// dépôt traque partout ailleurs.
///
/// Le verdict qu'on avait rendu sur elle part donc avec le blocage. **Pas le
/// sien sur nous** : qu'on se ravise ne rend pas à l'autre une décision qu'il
/// a prise. Concrètement, elle peut revenir dans notre sélection ; nous ne
/// revenons dans la sienne que s'il ne s'était jamais prononcé.
///
/// Ce qui ne revient pas : la conversation. Bloquer l'a supprimée, avec ses
/// messages, des deux côtés. Débloquer rouvre une porte, il ne ressuscite
/// rien — et le dire vaut mieux que de laisser espérer.
async fn unblock_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;

    let existant = block::Entity::find()
        .filter(block::Column::BlockerId.eq(claims.sub))
        .filter(block::Column::BlockedId.eq(id))
        .one(&state.db)
        .await?;

    // Rien à défaire : la réponse est la même que si on venait de le faire.
    // Un 404 ici ferait échouer un second appui sur un bouton qui a marché.
    let Some(existant) = existant else {
        return Ok(());
    };

    // Les deux ensemble : un blocage retiré sans son verdict laisse la
    // personne invisible, et un verdict retiré sans son blocage la laisse
    // bloquée. L'un sans l'autre est un état que rien ne décrit.
    state
        .db
        .transaction::<_, (), sea_orm::DbErr>(move |txn| {
            Box::pin(async move {
                block::Entity::delete_by_id(existant.id).exec(txn).await?;
                swipe::Entity::delete_many()
                    .filter(swipe::Column::ViewerId.eq(claims.sub))
                    .filter(swipe::Column::TargetId.eq(id))
                    .exec(txn)
                    .await?;
                Ok(())
            })
        })
        .await
        .map_err(|erreur| match erreur {
            sea_orm::TransactionError::Transaction(inner) => ApiError::from(inner),
            sea_orm::TransactionError::Connection(inner) => ApiError::from(inner),
        })?;

    Ok(())
}

/// Blocking is idempotent: someone pressing it twice is telling us the same
/// thing, and an error would read as though it had not worked.
async fn block_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;

    if claims.sub == id {
        return Err(ApiError::BadRequest(
            "On ne peut pas se bloquer soi-même.".into(),
        ));
    }

    let existing = block::Entity::find()
        .filter(block::Column::BlockerId.eq(claims.sub))
        .filter(block::Column::BlockedId.eq(id))
        .one(&state.db)
        .await?;

    if existing.is_some() {
        return Ok(());
    }

    block::ActiveModel {
        id: Set(Uuid::new_v4()),
        blocker_id: Set(claims.sub),
        blocked_id: Set(id),
        created_at: Set(Utc::now().into()),
    }
    .insert(&state.db)
    .await?;

    // Et le match s'en va avec — donc la conversation et ses messages, par la
    // cascade. Un blocage qui laisserait le fil ouvert ne serait qu'un filtre
    // sur le deck : la personne bloquée continuerait d'écrire, et ses messages
    // continueraient d'arriver. C'est l'outil qu'on utilise quand on est
    // harcelé ; il doit couper, pas masquer.
    //
    // Les verdicts restent, pour la même raison que dans `DELETE /matches` :
    // le deck exclut les profils déjà jugés, et les effacer ferait réapparaître
    // la personne qu'on vient de bloquer.
    let (lower, upper) = crate::entities::match_pair::ordered(claims.sub, id);
    match_pair::Entity::delete_many()
        .filter(match_pair::Column::LowerId.eq(lower))
        .filter(match_pair::Column::UpperId.eq(upper))
        .exec(&state.db)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn jour(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + chrono::Duration::days(n)
    }

    /// Les deux bornes égales rendent le comportement fixe. C'est ce dont se
    /// servent les suites qui doivent connaître le compte à l'avance, donc
    /// c'est ce qui tient la moitié des tests d'intégration debout.
    #[test]
    fn une_fourchette_plate_ne_varie_pas() {
        let qui = Uuid::new_v4();
        for n in 0..50 {
            assert_eq!(taille_du_jour(3, 3, qui, jour(n)), 3);
        }
    }

    /// Le compte reste dans la fourchette, bornes comprises.
    ///
    /// Les deux bouts sont ce qu'un modulo rate : `min` seul quand l'étendue
    /// est mal calculée, `max` jamais atteint quand elle est trop courte d'un.
    #[test]
    fn le_compte_couvre_la_fourchette_sans_la_deborder() {
        let mut vus = std::collections::BTreeSet::new();
        for _ in 0..200 {
            let qui = Uuid::new_v4();
            for n in 0..30 {
                let taille = taille_du_jour(2, 5, qui, jour(n));
                assert!(
                    (2..=5).contains(&taille),
                    "{taille} est hors de la fourchette"
                );
                vus.insert(taille);
            }
        }
        assert_eq!(
            vus,
            [2, 3, 4, 5].into_iter().collect(),
            "une fourchette dont un bout ne sort jamais n'est pas la fourchette annoncée"
        );
    }

    /// Le même jour rend le même compte. C'est ce qui permet de rattraper un
    /// tirage coupé en plein milieu sans viser un autre nombre.
    #[test]
    fn le_meme_jour_rend_le_meme_compte() {
        let qui = Uuid::new_v4();
        for n in 0..20 {
            let une_fois = taille_du_jour(1, 9, qui, jour(n));
            assert_eq!(taille_du_jour(1, 9, qui, jour(n)), une_fois);
        }
    }

    /// Deux personnes n'ont pas la même journée. Sans le brassage de
    /// l'identifiant, le compte ne dépendrait que de la date et tout le monde
    /// aurait le même — ce qui se remarque, et ce qui n'est pas ce qu'on a
    /// dit.
    #[test]
    fn deux_personnes_ne_suivent_pas_la_meme_suite() {
        let une = (0..40).map(|n| taille_du_jour(1, 9, Uuid::from_u128(1), jour(n)));
        let autre: Vec<u32> = (0..40)
            .map(|n| taille_du_jour(1, 9, Uuid::from_u128(2), jour(n)))
            .collect();
        assert_ne!(une.collect::<Vec<_>>(), autre);
    }

    /// Et le compte bouge d'un jour à l'autre pour une même personne : c'est
    /// exactement ce qui a été demandé, et une fonction qui rendrait la même
    /// valeur tout le temps passerait tous les autres tests de ce module.
    #[test]
    fn le_compte_bouge_dun_jour_a_lautre() {
        for graine in 1..200u128 {
            let qui = Uuid::from_u128(graine);
            let suite: std::collections::BTreeSet<u32> = (0..15)
                .map(|n| taille_du_jour(2, 5, qui, jour(n)))
                .collect();
            assert!(
                suite.len() > 1,
                "quinze jours d'affilée avec le même compte pour {qui}"
            );
        }
    }
}
