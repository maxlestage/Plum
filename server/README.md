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

## Ce qui est fait, ce qui ne l'est pas

**Fait** : santé, inscription, connexion, rafraîchissement avec rotation,
déconnexion globale, `/me`. Mots de passe en Argon2id, jetons de
rafraîchissement stockés en empreinte seulement, barrière 18+ vérifiée côté
serveur, et énumération des comptes fermée — une adresse inconnue et un
mauvais mot de passe répondent exactement la même chose.

**Pas fait** : profils, photos, deck, matchs, messagerie. Les photos
demanderont un stockage objet : le système de fichiers d'un dyno est éphémère
et ne peut pas les garder.

**À revoir avec plusieurs dynos** : les migrations tournent au démarrage. Avec
une seule dyno c'est correct ; avec plusieurs, deux se marcheraient dessus et
il faudra une phase `release`.
