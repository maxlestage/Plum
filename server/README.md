# Serveur Plum

API Rust (axum + SeaORM) pour l'application iOS. Déployée sur Heroku par image
Docker construite en CI.

## Le contrat vient du client

L'application iOS existait avant ce serveur, et c'est elle qui fixe le format :
clés en `snake_case`, dates en ISO 8601, enveloppe d'erreur `{message, code}`.
Ces conventions ne sont pas des préférences, ce sont des obligations — un
`profileCompleted` au lieu de `profile_completed` casse silencieusement chaque
décodage côté client.

Deux pièges déjà rencontrés, tous deux couverts par des tests :

- **`birth_date` arrive comme horodatage complet**, pas comme date nue :
  l'encodeur Swift traite toutes les `Date` de la même façon.
- **`gender` vaut `nonBinary`**, pas `non_binary` : Swift ne transforme pas les
  valeurs brutes de ses énumérations.

## Lancer en local

```bash
createdb plum
export DATABASE_URL=postgresql://localhost/plum
export JWT_SECRET=$(openssl rand -hex 32)
cargo run
```

Les migrations s'appliquent au démarrage. Le processus refuse de démarrer sur
une configuration invalide, avec un message qui nomme la variable fautive —
un dyno qui meurt au boot vaut mieux qu'un dyno qui répond mal.

## Tests

```bash
cargo test                                   # logique pure, sans base
TEST_DATABASE_URL=postgresql://localhost/plum_test cargo test   # tout
```

Sans `TEST_DATABASE_URL` les tests d'intégration sautent en le disant. La CI
pose `REQUIRE_TEST_DATABASE=1`, ce qui les fait échouer plutôt que sauter si le
service Postgres n'a pas démarré : dix tests verts qui n'ont rien exécuté sont
pires qu'un rouge.

Chaque test ouvre son propre pool, et ce n'est pas du gaspillage :
`#[tokio::test]` donne à chacun son propre runtime, et une connexion sqlx
attache sa socket au pilote d'E/S du runtime qui l'a ouverte. Un pool partagé
distribue donc aux tests suivants des connexions dont le pilote est mort avec
le premier runtime — elles n'aboutissent jamais et l'acquisition expire au bout
de vingt secondes.

Les migrations, elles, ne doivent tourner qu'une fois : un verrou consultatif
Postgres les sérialise. Le verrou vaut mieux qu'une cellule locale au processus
parce qu'il tient aussi entre deux `cargo test` lancés en même temps.

## Le parcours de bout en bout

```bash
DATABASE_URL=… JWT_SECRET=… PORT=8099 ./target/release/plum-server &
PLUM_API=http://127.0.0.1:8099/api/v1 ../Scripts/parcours.sh
```

Les tests d'intégration prouvent la logique en passant par la couche `Router`
en mémoire. Ce script prouve l'assemblage — le binaire, la configuration, le
port, la base — ce qu'aucun d'eux ne touche. Il a déjà servi : après avoir
ajouté le deck, il a répondu 404 partout, parce que le binaire qui tournait
datait d'avant.

## Déploiement

L'image est construite dans GitHub Actions, jamais sur Heroku — qui plafonne
ses builds à 15 minutes, ce qu'une compilation Rust en release dépasse sans
peine. Heroku ne reçoit qu'une image finie, et la release ne prend que
quelques secondes.

Pour armer le déploiement, deux secrets dans le dépôt GitHub :

| Secret | Où le trouver |
| --- | --- |
| `HEROKU_API_KEY` | Compte Heroku → Account settings → API Key |
| `HEROKU_APP_NAME` | Le nom de l'application Heroku |

Tant qu'ils sont absents, le workflow construit et teste l'image puis saute le
déploiement en le signalant. Rien ne casse avant que Heroku n'existe.

### Variables à poser sur Heroku

| Variable | Rôle |
| --- | --- |
| `DATABASE_URL` | Posée par l'add-on Postgres. Sa forme `postgres://` est réécrite au démarrage, SeaORM n'acceptant que `postgresql://`. |
| `JWT_SECRET` | 32 caractères minimum, sinon refus de démarrer. |
| `ACCESS_TOKEN_TTL_MINUTES` | 15 par défaut. |
| `REFRESH_TOKEN_TTL_DAYS` | 60 par défaut. |
| `REDIS_URL` | **Facultatif.** Sans lui, la limitation de débit compte dans le processus — donc par dyno, ce qui est plus faible mais démarre sans add-on. Heroku Key-Value Store présente un certificat auto-signé : l'URL `rediss://` doit porter `#insecure`, et le serveur le signale au démarrage si elle ne l'a pas. |
| `DATABASE_MAX_CONNECTIONS` | 10 par défaut. Heroku Postgres Essential-0 en autorise **20 pour tout le compte**, pas par dyno : dépasser ce plafond produit une erreur qui ne nomme ni le plan ni la limite. |

## Ce qui est fait, ce qui ne l'est pas

**Fait** : santé, inscription, connexion, rafraîchissement avec rotation,
déconnexion globale, `/me`, et limitation de débit sur l'inscription et la
connexion — dix tentatives par quart d'heure et par adresse, comptées avant la
vérification du mot de passe pour qu'un limiteur ne révèle pas quelles
suppositions approchaient. Mots de passe en Argon2id, jetons de
rafraîchissement stockés en empreinte seulement, barrière 18+ vérifiée côté
serveur, et énumération des comptes fermée — une adresse inconnue et un
mauvais mot de passe répondent exactement la même chose.

**Fait aussi** : le profil et ses préférences — `GET`/`PATCH /me/profile`,
`GET`/`PATCH /me/preferences`, `POST /me/profile/complete`,
`PATCH /me/location`.

Quelques points qui ne se devinent pas à la lecture des routes :

- Un `PATCH` de profil ne porte que ce qui a changé. Une clé absente veut dire
  « laisse ce champ tranquille », jamais « efface-le » — sans quoi modifier sa
  ville effacerait sa description.
- Les centres d'intérêt sont coupés, vidés de leurs blancs et dédupliqués sans
  tenir compte de la casse, en gardant l'ordre choisi. « Cinéma » deux fois sur
  une carte ressemble à un bug parce que c'en est un.
- Les bornes des préférences (18 ≤ âge ≤ 99, distance 1..300) sont appliquées
  **trois fois** : par le client, par le serveur, et par une contrainte `CHECK`.
  Le contrôle client est une courtoisie ; celui de la base est là pour la
  prochaine route qui écrira dans cette table en oubliant la règle. Un test
  d'intégration insère des lignes illégales en SQL direct et vérifie que c'est
  bien la contrainte nommée qui les refuse.
- `POST /me/profile/complete` est idempotent : l'app peut le rejouer après une
  connexion coupée sans savoir si le premier essai a abouti.
- `PATCH /me/location` rafraîchit aussi l'horodatage d'activité, qui est ce qui
  allume la pastille verte.

**Fait aussi** : le deck — `GET /discovery/deck`, `POST /discovery/swipes`,
`POST /discovery/rewind`, `POST /profiles/{id}/report`,
`POST /profiles/{id}/block`.

Les points qui ne se lisent pas dans la liste des routes :

- **Le curseur n'est pas un décalage.** Un deck bouge pendant qu'on le lit :
  d'autres gens jugent, des profils apparaissent et disparaissent. Un `OFFSET`
  répéterait ou sauterait des cartes en silence. La pagination compare le
  n-uplet `(distance, id)`, ce que Postgres sait faire nativement. Les deux
  écritures naïves échouent précisément sur les ex æquo : `distance >` les
  saute, `distance >=` les répète. Un test pagine sept candidats placés au
  même point, deux par deux, et vérifie qu'aucun n'est vu deux fois ni oublié.
- **Tout est exclu dans la requête**, pas après coup : déjà jugé, bloqué dans
  un sens ou dans l'autre, masqué, hors bornes d'âge ou de distance. Filtrer
  une page déjà récupérée rendrait des pages courtes ou vides alors qu'il
  reste des candidats.
- **La distance est calculée en SQL pur**, sans PostGIS ni `earthdistance` :
  deux extensions de moins à installer sur Heroku Postgres depuis un
  téléphone, pour une formule.
- **Un profil sans position n'est pas montré à quelqu'un qui en a une.** Un
  rayon que l'on choisit doit vouloir dire quelque chose, et un profil dont la
  position est inconnue ne peut pas prétendre être dans les 20 km. La
  conséquence, qui est une décision et non un effet de bord : **un profil
  n'est découvrable qu'une fois sa position envoyée**. À l'inverse, un
  visiteur sans position n'a pas de rayon à appliquer et voit tout le monde,
  plutôt qu'un deck vide.
- **`interestedIn` n'a que trois valeurs pour quatre genres.** « women » et
  « men » ne retiennent que `woman` et `man` : un profil non binaire n'est
  atteint que par « everyone ». C'est une conséquence du modèle du client, et
  un test la fixe explicitement pour qu'elle reste un choix visible.
- **Un match appartient à une paire, pas à un sens.** Une seule ligne, plus
  petit identifiant d'abord, avec une clé unique sur la paire ordonnée et une
  contrainte `CHECK` qui garantit l'ordre — sans quoi une ligne écrite à
  l'envers échapperait à la clé unique et créerait le doublon qu'elle existe
  pour empêcher.
- **Le retour en arrière supprime le verdict** au lieu de le marquer défait :
  le deck exclut tout profil déjà jugé, donc une ligne laissée en place
  garderait la carte cachée et le retour semblerait ne rien faire.
- **`likesRemaining` vaut toujours `null`.** Un plafond quotidien est ce qu'une
  application de rencontres vend d'ordinaire ; il n'y a rien à vendre ici.
  Inventer un quota reviendrait à deviner un modèle économique.

**Pas fait** : photos, messagerie, et la liste des matchs (ils sont créés et
annoncés, mais rien ne les relit encore). `photos` est donc toujours un
tableau vide dans les réponses — présent parce que le modèle Swift le déclare
non optionnel, vide parce que les photos demandent un stockage objet : le
système de fichiers d'un dyno est éphémère et ne peut pas les garder. Servir
des téléversements qui disparaissent au prochain redémarrage serait pire que
de ne pas les servir.

**À revoir avec plusieurs dynos** : les migrations tournent au démarrage. Avec
une seule dyno c'est correct ; avec plusieurs, deux se marcheraient dessus et
il faudra une phase `release`.
