# Plum depuis un téléphone : backend Heroku et app iOS sans Mac

## La contrainte, dite franchement

**Un téléphone ne peut pas compiler une application iOS.** Xcode n'existe pas
sur iOS et n'existera pas. Ce n'est pas un obstacle, parce que rien n'oblige à
compiler là où l'on travaille : la CI mise en place dans ce dépôt compile et
teste déjà l'application sur des machines macOS louées à l'heure. Il suffit de
la prolonger jusqu'à TestFlight.

**Heroku n'héberge pas d'application iOS.** Heroku fera tourner le backend Rust.
L'application, elle, arrive sur l'iPhone par TestFlight.

Donc : deux chaînes distinctes, toutes deux pilotables au doigt.

## Ce que le téléphone fait réellement

| Besoin | Outil, sur téléphone |
| --- | --- |
| Écrire et modifier du code | Claude Code sur le web (claude.ai/code) — ce qui produit ce texte |
| Relire et fusionner les PR | GitHub, app iOS ou web mobile |
| Déclencher un build | GitHub Actions : bouton « Run workflow » |
| Variables d'environnement, journaux, redémarrages | Tableau de bord Heroku, web mobile |
| Gérer les builds et testeurs | App Store Connect, app iOS gratuite |
| Installer l'app | TestFlight |
| Git ponctuel (branche, commit) | Working Copy, app iOS |

Aucune de ces étapes n'exige un ordinateur.

---

## Chaîne 1 — Backend Rust sur Heroku

### Le piège à éviter d'emblée

Heroku impose **15 minutes** de temps de build. Une compilation Rust en release
d'un axum + SeaORM les dépasse sans peine, surtout au premier build. Compiler
sur Heroku est donc un cul-de-sac.

**La sortie** : construire l'image Docker dans GitHub Actions — qui n'a pas
cette limite et sait mettre en cache les couches de dépendances — puis la
pousser au registre de conteneurs Heroku et déclencher la release par API.
Heroku ne fait plus que lancer une image déjà prête. Le déploiement tombe à
quelques secondes, et tout se pilote depuis GitHub.

### Forme du projet

```
server/
├── Cargo.toml            axum 0.7, sea-orm 1.x, tokio, tower-http, jsonwebtoken
├── Dockerfile            build multi-étages, image finale distroless
├── migration/            migrations SeaORM (crate séparée, convention sea-orm-cli)
└── src/
    ├── main.rs           router, état partagé, port depuis $PORT
    ├── auth/             inscription, connexion, rafraîchissement, JWT
    ├── profiles/         profil, photos, préférences, position
    ├── discovery/        deck, swipes, rewind, signalement, blocage
    ├── matches/          matchs, conversations
    ├── chat/             messages REST + WebSocket
    └── entities/         entités SeaORM générées
```

Les routes sont déjà spécifiées : le README de l'app iOS les liste, et le
client les appelle en snake_case avec des dates ISO 8601. Le contrat existe
avant le serveur, ce qui est l'ordre confortable.

### Points où Heroku impose ses règles

- **Port** : écouter sur `$PORT`, jamais un port fixe.
- **Base** : l'add-on Heroku Postgres fournit `DATABASE_URL`. Attention, il
  donne parfois une URL en `postgres://` que SeaORM veut en `postgresql://` —
  à normaliser au démarrage.
- **Système de fichiers éphémère** : un dyno perd ses fichiers à chaque
  redéploiement. **Les photos ne peuvent pas être stockées sur le dyno.**
  L'app envoie du multipart sur `POST /me/photos` ; le serveur doit reverser
  vers S3 (ou Cloudflare R2, moins cher, API compatible) et ne renvoyer que
  l'URL. C'est la seule vraie surprise d'architecture imposée par Heroku.
- **WebSocket** : supporté sur les dynos standards, avec un délai d'inactivité
  de 55 secondes. Le client sait déjà se reconnecter avec un recul
  exponentiel ; il faut y ajouter un ping serveur toutes les ~30 secondes.
- **Migrations** : phase `release` du Procfile, pour qu'elles tournent avant
  la bascule et non dans chaque dyno au démarrage.

### Coût réel

Heroku n'a plus d'offre gratuite depuis novembre 2022.

| Poste | Prix mensuel |
| --- | --- |
| Dyno Eco | 5 $ (mis en veille après 30 min d'inactivité) |
| Dyno Basic | 7 $ (jamais en veille — préférable pour un WebSocket) |
| Postgres Essential-0 | 5 $ |
| Stockage objet (R2/S3) | quelques centimes au début |

Compter **~12 $/mois** pour quelque chose d'utilisable. Un dyno Eco qui
s'endort coupe les WebSockets : pour une messagerie, le Basic se justifie.

---

## Chaîne 2 — L'app iOS jusqu'à TestFlight, sans Mac

### Signature

Deux approches. **La bonne ici** : signature dans le nuage via une clé API App
Store Connect et `xcodebuild -allowProvisioningUpdates`. Xcode crée et
renouvelle lui-même les certificats et profils sur le runner. Rien à gérer à
la main, rien à stocker en dehors d'une clé.

L'alternative classique — fastlane match avec un dépôt privé de certificats —
suppose de générer les certificats quelque part la première fois, ce qui est
précisément ce qu'un téléphone fait mal.

### Préparation, une seule fois, depuis Safari mobile

1. Apple Developer : adhérer (99 €/an).
2. App Store Connect → Users and Access → Integrations → clé API, rôle *App
   Manager*. Télécharger le `.p8` — **une seule fois**, il n'est plus jamais
   proposé.
3. Enregistrer l'identifiant `app.plum.ios` et créer la fiche d'application.
4. Dans GitHub → Settings → Secrets : `ASC_KEY_ID`, `ASC_ISSUER_ID`,
   `ASC_KEY_P8` (le contenu du fichier), `TEAM_ID`.

Tout cela se fait au doigt, sans confort mais sans obstacle.

### Le workflow

Un second workflow, déclenché à la main ou sur étiquette :

```
archive → export ipa → envoi App Store Connect → TestFlight
```

L'export a besoin d'un `ExportOptions.plist` avec `method: app-store-connect`
et `signingStyle: automatic`. L'envoi passe par fastlane `pilot` ou
`xcrun altool --upload-app`, authentifié par la clé API.

Le générateur de projet devra accepter `DEVELOPMENT_TEAM` — une ligne à
ajouter, la même mécanique que les autres réglages.

### Ce qui change dans l'app

L'application pointe aujourd'hui sur `127.0.0.1` par défaut et démarre en mode
démo. Pour une build TestFlight il faut une configuration Release qui vise
l'URL Heroku, sans mode démo. Les variables de schéma ne s'appliquent qu'au
lancement depuis Xcode : il faut donc une valeur compilée, pas une variable
d'environnement.

---

## Ordre des travaux

1. **Sortir l'app iOS de l'état « dépend d'un serveur absent »** : configuration
   Release pointant sur l'URL Heroku, `DEVELOPMENT_TEAM` dans le générateur.
2. **Backend, tranche verticale** : santé, inscription, connexion,
   rafraîchissement. Déployé et joignable avant d'écrire la suite.
3. **Chaîne d'image** : Dockerfile, workflow GitHub qui construit et pousse au
   registre Heroku, release par API. Vérifiée sur la tranche ci-dessus.
4. **Profils et photos** avec le stockage objet — c'est là que se cache le
   travail réel.
5. **Deck, swipes, matchs.**
6. **Messagerie** REST puis WebSocket avec ping.
7. **Workflow TestFlight**, une fois qu'il y a quelque chose à essayer.

La tranche 2 avant tout le reste : un backend déployé qui répond à
`/health` prouve la chaîne entière. Tout ce qui suit n'est plus que du métier.

## Deux choses que je ne peux pas faire à votre place

- **Accepter les contrats Apple et payer.** Adhésion, contrats bancaires et
  fiscaux dans App Store Connect : c'est nominatif.
- **Récupérer la clé `.p8`.** Apple ne la propose qu'une fois. Si elle est
  perdue, il faut en régénérer une.

Le reste — le code, les workflows, le Dockerfile, les migrations — je peux
l'écrire et le pousser d'ici.
