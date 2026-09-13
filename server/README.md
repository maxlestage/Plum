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

**Pas fait** : profils, photos, deck, matchs, messagerie. Les photos
demanderont un stockage objet : le système de fichiers d'un dyno est éphémère
et ne peut pas les garder.

**À revoir avec plusieurs dynos** : les migrations tournent au démarrage. Avec
une seule dyno c'est correct ; avec plusieurs, deux se marcheraient dessus et
il faudra une phase `release`.
