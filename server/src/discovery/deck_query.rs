use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr, FromQueryResult, Statement};
use uuid::Uuid;

use super::types::Cursor;

/// One candidate, already filtered and measured by the database.
#[derive(Debug, FromQueryResult)]
pub struct Candidate {
    pub id: Uuid,
    pub display_name: String,
    pub birth_date: chrono::NaiveDate,
    pub gender: String,
    pub bio: String,
    pub city: String,
    pub interests: Vec<String>,
    pub last_active_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    /// Absent when either side has no position. The client shows the city
    /// alone rather than a made-up number.
    pub distance_km: Option<f64>,
    /// What the cursor is compared against: the distance, or a value past
    /// every real one when there is none, so the ordering has no nulls to
    /// trip over and profiles without a position simply come last.
    pub sort_km: f64,
}

/// Straight-line distance in kilometres, in plain SQL.
///
/// No PostGIS and no `earthdistance`: both are extensions to install on
/// Heroku Postgres from a phone, for one formula.
const HAVERSINE: &str = "6371.0 * 2 * asin(sqrt(
    power(sin(radians(c.latitude - v.latitude) / 2), 2)
  + cos(radians(v.latitude)) * cos(radians(c.latitude))
  * power(sin(radians(c.longitude - v.longitude) / 2), 2)))";

/// La distance, ramenée aux paliers que le client affiche.
///
/// **Ce n'est pas de la présentation, c'est la mesure de protection.** Une
/// distance exacte suffit à retrouver une adresse : il suffit de se placer
/// successivement à trois endroits — `PATCH /me/location` accepte n'importe
/// quelle position — de lire trois distances précises, et de trianguler.
/// C'est une attaque connue contre les applications de rencontres, et elle a
/// des conséquences bien réelles.
///
/// L'arrondi est donc fait ici, avant que la valeur ne quitte la base : le
/// client n'a jamais accès à mieux qu'un palier, et la page Confidentialité
/// qui promet « jamais assez pour trouver quelqu'un » dit enfin vrai.
///
/// Les paliers reprennent exactement ceux de `DistanceFormatter` côté Swift,
/// pour que l'affichage soit identique : moins d'un kilomètre, puis au
/// kilomètre, puis par tranches de cinq.
const COARSE: &str = "CASE
    WHEN %D% < 1  THEN 0.5
    WHEN %D% < 10 THEN round((%D%)::numeric)::double precision
    ELSE round((%D% / 5)::numeric)::double precision * 5
  END";

/// Everyone this viewer could still be shown, nearest first.
///
/// Written as one statement on purpose. Every exclusion here — already
/// judged, blocked either way, hidden themselves, outside the age range or
/// the distance — is a `NOT EXISTS` or a predicate rather than a filter
/// applied to a page after it was fetched. Filtering in the application would
/// return short pages, or empty ones, while candidates remained.
pub async fn deck(
    db: &impl ConnectionTrait,
    viewer: Uuid,
    limit: u32,
    cursor: Option<Cursor>,
) -> Result<Vec<Candidate>, DbErr> {
    eligible(db, viewer, limit, cursor, None).await
}

/// Les mêmes écarts, restreints à une poignée de personnes.
///
/// Sert à relire une sélection déjà tirée : de ces trois-là, qui peut encore
/// être montré ? La question se posait avant en demandant les deux cents plus
/// proches et en cherchant les trois dedans — ce qui marche tant que les trois
/// sont dans les deux cents. Quelqu'un qui voyage suffisamment pour que sa
/// sélection du matin se retrouve au-delà du deux-centième rang verrait sa
/// sélection se vider sans avoir rien décidé, et rien ne le lui dirait.
///
/// Passer les identifiants dans la requête plutôt que filtrer après coup :
/// c'est la même phrase SQL, donc les mêmes exclusions, sans plafond arbitraire
/// et sans seconde version des règles qui dériverait de la première.
pub async fn among(
    db: &impl ConnectionTrait,
    viewer: Uuid,
    ids: &[Uuid],
) -> Result<Vec<Candidate>, DbErr> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    eligible(db, viewer, ids.len() as u32, None, Some(ids)).await
}

async fn eligible(
    db: &impl ConnectionTrait,
    viewer: Uuid,
    limit: u32,
    cursor: Option<Cursor>,
    restrict: Option<&[Uuid]>,
) -> Result<Vec<Candidate>, DbErr> {
    // A value no real distance reaches: half the Earth's circumference is
    // about 20 004 km.
    const NO_POSITION: f64 = 1.0e9;

    let keyset = if cursor.is_some() {
        // Row-value comparison, which Postgres evaluates left to right. The
        // two obvious spellings are both wrong on ties: `sort_km > $` skips
        // everyone at the same distance as the last card of the previous
        // page, and `>=` shows them again.
        "AND (ranked.sort_km, ranked.id) > ($4, $5)"
    } else {
        ""
    };

    let coarse = COARSE.replace("%D%", HAVERSINE);

    // `$6` et au-delà : après les cinq que le curseur occupe déjà, pour que
    // leurs numéros ne bougent pas selon qu'il est là ou non.
    let restreint = match restrict {
        Some(ids) => {
            let places: Vec<String> = (0..ids.len()).map(|i| format!("${}", i + 6)).collect();
            format!("AND c.id IN ({})", places.join(", "))
        }
        None => String::new(),
    };

    let sql = format!(
        "WITH v AS (
            SELECT p.id, p.latitude, p.longitude, p.birth_date,
                   COALESCE(pr.interested_in, 'everyone')  AS interested_in,
                   COALESCE(pr.min_age, 18)                AS min_age,
                   COALESCE(pr.max_age, 45)                AS max_age,
                   COALESCE(pr.max_distance_km, 50)        AS max_distance_km
            FROM profiles p
            LEFT JOIN preferences pr ON pr.id = p.id
            WHERE p.id = $1
        ),
        ranked AS (
            SELECT c.id, c.display_name, c.birth_date, c.gender, c.bio, c.city,
                   c.interests, c.last_active_at,
                   CASE WHEN v.latitude IS NULL OR c.latitude IS NULL THEN NULL
                        ELSE {coarse} END AS distance_km,
                   -- Le tri se fait sur la valeur arrondie, et pas seulement
                   -- l'affichage : le curseur de pagination transporte cette
                   -- valeur jusqu'au client, et une distance exacte y fuirait
                   -- tout aussi bien que dans la réponse.
                   COALESCE(
                     CASE WHEN v.latitude IS NULL OR c.latitude IS NULL THEN NULL
                          ELSE {coarse} END,
                     {NO_POSITION}) AS sort_km,
                   v.max_distance_km,
                   (v.latitude IS NOT NULL) AS viewer_located
            FROM profiles c
            CROSS JOIN v
            LEFT JOIN preferences cp ON cp.id = c.id
            WHERE c.id <> v.id
              -- Hiding yourself removes you from other people's decks and
              -- changes nothing about your own.
              AND COALESCE(cp.show_me_on_plum, true)
              AND NOT EXISTS (
                    SELECT 1 FROM swipes s
                    WHERE s.viewer_id = v.id AND s.target_id = c.id)
              -- Un compte fermé par la modération disparaît des decks. Le
              -- profil existe toujours — une suspension se lève, et les
              -- messages qu'elle sanctionne sont la trace de la décision —
              -- mais plus personne ne le croise.
              AND NOT EXISTS (
                    SELECT 1 FROM users u
                    WHERE u.id = c.id AND u.suspended_at IS NOT NULL)
              -- Blocking cuts both ways, so the deck asks about both.
              AND NOT EXISTS (
                    SELECT 1 FROM blocks b
                    WHERE (b.blocker_id = v.id AND b.blocked_id = c.id)
                       OR (b.blocker_id = c.id AND b.blocked_id = v.id))
              AND (v.interested_in = 'everyone'
                   OR (v.interested_in = 'women' AND c.gender = 'woman')
                   OR (v.interested_in = 'men'   AND c.gender = 'man'))
              AND EXTRACT(YEAR FROM age(c.birth_date))
                    BETWEEN v.min_age AND v.max_age
              {restreint}
        )
        SELECT id, display_name, birth_date, gender, bio, city, interests,
               last_active_at, distance_km, sort_km
        FROM ranked
        -- A radius someone chose has to mean something. A profile whose
        -- position is unknown cannot be shown to satisfy « within 20 km » —
        -- it might be anywhere — so it is left out for a viewer who has a
        -- position of their own.
        --
        -- A viewer with no position has no radius to apply, and sees
        -- everyone: the alternative is an empty deck until they grant
        -- location. The practical consequence, worth saying out loud, is that
        -- a profile is only discoverable once it has sent a position.
        WHERE (NOT viewer_located
               OR (distance_km IS NOT NULL AND distance_km <= max_distance_km))
        {keyset}
        ORDER BY ranked.sort_km, ranked.id
        LIMIT $2 OFFSET $3"
    );

    let mut values: Vec<sea_orm::Value> = vec![
        viewer.into(),
        i64::from(limit).into(),
        0_i64.into(), // Never paged by offset; kept so $4/$5 keep their number.
    ];
    if let Some(cursor) = cursor {
        values.push(cursor.sort_km.into());
        values.push(cursor.id.into());
    } else if restrict.is_some() {
        // Les deux places du curseur restent occupées : sans elles, `$6`
        // désignerait la première personne restreinte et la requête lirait
        // les paramètres décalés de deux rangs.
        values.push(0.0_f64.into());
        values.push(Uuid::nil().into());
    }
    if let Some(ids) = restrict {
        for id in ids {
            values.push((*id).into());
        }
    }

    Candidate::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        &sql,
        values,
    ))
    .all(db)
    .await
}
