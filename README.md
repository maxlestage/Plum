# Plum

Plum est une application de rencontres pour iOS. Cette version est une
application **iOS native, écrite entièrement en Swift** : SwiftUI pour l'interface,
`async`/`await` et acteurs pour la concurrence, `URLSession` pour le réseau.
Aucune dépendance tierce.

## Ouvrir le projet

```bash
open Plum.xcodeproj
```

Le projet cible **iOS 17.0, iPhone uniquement, en portrait** — le deck plein
écran et la barre d'onglets sont un design de téléphone, et revendiquer l'iPad
sans mise en page iPad est un motif de rejet connu. Il se construit avec
Xcode 15 ou plus récent. Le
schéma `Plum` est partagé et démarre en **mode démo** : l'application est
entièrement utilisable dans le simulateur, avec des données en mémoire, sans
qu'aucun serveur ne tourne.

Pour la brancher sur une vraie API, dans le schéma (`Product ▸ Scheme ▸ Edit
Scheme… ▸ Run ▸ Arguments`) :

| Variable            | Rôle                                       |
| ------------------- | ------------------------------------------ |
| `PLUM_DEMO_MODE`    | `1` pour les données en mémoire. Décochez-la pour viser le réseau. |
| `PLUM_API_BASE_URL` | Racine REST, par défaut `http://127.0.0.1:8080/api/v1`. |
| `PLUM_WS_BASE_URL`  | Socket de messagerie, par défaut `ws://127.0.0.1:8080/ws`. |
| `PLUM_TERMS_URL` · `PLUM_PRIVACY_URL` · `PLUM_SUPPORT_URL` | Pages légales, pour pointer une préproduction. |

## Ce que fait l'application

- **Inscription et connexion** — jetons JWT, rafraîchissement automatique,
  stockage dans le trousseau, et une barrière 18+ vérifiée côté client. Une
  session invalidée par le serveur ramène l'interface à l'écran d'accueil au
  lieu de la laisser empiler des erreurs.
- **Onboarding** — un compte neuf passe par trois étapes (photo obligatoire,
  ville et bio, critères) avant d'atteindre le deck. `User.profileCompleted`
  décide du routage à chaque lancement.
- **Découverte** — un deck de cartes que l'on balaie à droite (j'aime), à
  gauche (non) ou vers le haut (coup de cœur), avec annulation du dernier
  passe, signalement et blocage depuis chaque carte. Un chevron ouvre le
  profil complet — toutes les photos, la bio entière, tous les centres
  d'intérêt — avec les trois mêmes verdicts à portée, pour décider sur autre
  chose que trois lignes. La position est demandée
  pendant l'onboarding, à côté du réglage de distance, et repoussée à chaque
  lancement : c'est elle qui fait exister les distances affichées. Un refus
  masque les distances sans rien casser d'autre.
- **Matchs** — l'écran de célébration, la rangée des matchs sans conversation
  entamée, puis la boîte de réception.
- **Messagerie** — historique paginé, envoi optimiste (la bulle apparaît
  avant la réponse du serveur), accusés de lecture, indicateur de saisie et
  reconnexion automatique du WebSocket. Signalement, blocage et retrait du
  match sont accessibles depuis la conversation elle-même, pas seulement
  depuis la carte.
- **Profil et réglages** — photos (dont le choix de la couverture), bio,
  centres d'intérêt, critères de recherche, déconnexion et suppression de
  compte. Modifier les critères rafraîchit le deck immédiatement, alors même
  qu'il vit dans un autre onglet resté en mémoire.

## Le site

[`web/`](web/README.md) — React + TypeScript + Vite. Servi par le même dyno que
l'API, à la racine, avec les pages légales que l'App Store réclame. Un second
dyno coûterait plus cher que le premier pour quelques fichiers statiques.

## Le serveur

Le dépôt contient désormais aussi l'API que cette application attend :
[`server/`](server/README.md), en Rust (axum + SeaORM), déployée sur Heroku par
image Docker construite en CI. La tranche d'authentification est faite et
testée ; le reste est à écrire.

[`docs/plan-heroku.md`](docs/plan-heroku.md) décrit comment tout cela se pilote
depuis un téléphone, sans ordinateur — y compris ce qui ne peut pas l'être et
comment on contourne.

## Architecture

```
Plum/
├── App/            Composition root, gate d'authentification, session
├── Models/         Types du domaine, Codable, alignés sur l'API
├── Networking/     APIClient (acteur), Endpoint, trousseau, WebSocket
├── Services/       Une façade par domaine + doublures en mémoire
├── Features/       Un dossier par écran : vue + view model
├── DesignSystem/   Palette, typographie, composants réutilisables
├── Utilities/      Dates, haptique, état d'écran
└── Resources/      Catalogue d'assets (couleurs claires et sombres)
```

Chaque service est un protocole, avec une implémentation réseau et une
implémentation en mémoire (`DemoServices.swift`). `AppEnvironment` choisit
l'une ou l'autre au lancement, ce qui rend les écrans testables et les
aperçus Xcode utilisables hors ligne.

Les view models sont des classes `@MainActor @Observable` : elles portent
l'état et les règles, les vues ne portent que la mise en page et les gestes.

### Réseau

Les lectures sont rejouées jusqu'à trois fois sur une erreur serveur, une
coupure réseau ou un 429 — en respectant l'en-tête `Retry-After` quand il est
là, sinon avec un recul de 300 ms puis 900 ms. Les écritures ne sont **jamais**
rejouées : un swipe ou un message renvoyé serait compté deux fois.

### L'API attendue

Le client parle à une API REST en `snake_case` avec des dates ISO 8601 :

```
POST   /auth/sign-up · /auth/sign-in · /auth/refresh · /auth/sign-out
GET    /me · /me/profile · /me/preferences
PATCH  /me/profile · /me/preferences · /me/photos/order · /me/location
POST   /me/photos                       (multipart)
POST   /me/profile/complete             (fin de l'onboarding)
GET    /discovery/deck?limit=&cursor=
POST   /discovery/swipes · /discovery/rewind
GET    /matches · POST /matches/{id}/conversation
GET    /conversations · /conversations/{id}/messages
POST   /conversations/{id}/messages · /conversations/{id}/read
WS     /ws                              (message · read · typing · match)
```

`POST /conversations/{id}/messages` reçoit un `client_id` : le serveur doit
persister le message **sous cet identifiant**. C'est ce qui permet à la bulle
optimiste, à la réponse REST et à l'écho du socket de désigner le même message
— sans quoi un envoi peut s'afficher deux fois selon l'ordre d'arrivée.

## Tests

`⌘U` dans Xcode, ou :

```bash
xcodebuild test -scheme Plum -destination 'platform=iOS Simulator,name=iPhone 15'
```

Deux bundles tournent sous le même schéma.

`PlumTests` couvre la logique : décodage du format de l'API, construction des
requêtes, rafraîchissement des jetons (y compris le 401 inattendu et l'échec
qui déconnecte), seuils du geste de balayage, validation des formulaires,
envoi optimiste des messages, pagination et barrières de l'onboarding.

`PlumUITests` lance l'application en mode démo et la traverse : connexion,
deck, envoi d'un message, profil. Rien dans les tests unitaires n'attraperait
un écran qui plante à l'affichage ou un bouton branché sur rien.

## Intégration continue

`.github/workflows/ci.yml` fait deux choses à chaque PR :

- sur Ubuntu, il valide le graphe d'objets du projet et vérifie que le
  `pbxproj` committé correspond bien aux sources (la régénération ne doit
  produire aucun diff) ;
- sur macOS, il construit l'application et exécute les deux suites de tests
  sur un simulateur iPhone choisi à l'exécution par `Scripts/ci_test.sh`,
  plutôt que sur un nom de modèle codé en dur qu'une nouvelle image de runner
  casserait, et liste les avertissements du compilateur à chaque build.

## Les noms affichés

L'**application iPhone** s'appelle **Plum ‣** : sous l'icône, en haut de la
sélection, et dans la ligne que l'app Réglages lui donne. La marque s'arrête
là. La montre et le widget s'appellent **Plum**, comme le site, le serveur et
le dépôt — un glyphe sur un cadran de quarante millimètres s'afficherait
surtout tronqué, et la galerie de widgets range ses vignettes sous un nom.

Ce que le renommage ne touche pas : `PRODUCT_NAME` reste `Plum`, donc le
paquet reste `Plum.app`, le module Swift reste `Plum`, l'identifiant reste
`app.plum.ios` et l'hôte de tests continue de trouver son binaire. On renomme
ce qui s'affiche, pas ce que le système manipule.

Chaque nom vit dans deux mondes qui ne se lisent pas l'un l'autre —
`PlumBrand.name` et `PlumBrand.displayName` côté Swift,
`INFOPLIST_KEY_CFBundleDisplayName` côté projet Xcode, une déclaration par
cible et par configuration. Un renommage fait d'un seul côté compile, passe
tous les tests, et produit une application dont l'icône et l'en-tête portent
deux noms différents. `Scripts/check_app_name.py` vérifie cible par cible que
chacune porte celui des deux qui lui revient, refuse qu'un nom soit écrit en
dur dans une vue, et tourne en CI.

Le caractère est écrit tel quel dans le `pbxproj`, et pas en `\U2023` comme
Xcode le ferait lui-même : les deux formes se lisent pareil pour lui, mais une
seule se lit aussi en ouvrant le fichier. `check_app_name.py` refuse la forme
échappée, sans quoi on y reviendrait sans que rien ne le dise.

Ce contrôle-là compare des sources entre elles : il prouve que la recette est
cohérente, pas que le plat lui ressemble. Entre les deux il y a Xcode, qui doit
porter le caractère jusqu'au paquet — s'il le mangeait ou le réécrivait, ni la
compilation, ni les tests, ni les contrôles statiques ne s'en plaindraient.

Le même script, avec `--paquet`, ouvre les `Info.plist` du `Plum.app` réellement construit — le sien, celui de la
montre et celui du widget — et lit ce que chacun dit :

```bash
python3 Scripts/check_app_name.py --paquet build/DerivedData/Build/Products/Debug-iphonesimulator/Plum.app
```

C'est ce que fait l'étape « Le paquet construit porte le bon nom » après le
build iOS — et c'est pour ça que `Scripts/ci_test.sh` impose un
`-derivedDataPath` : sans lui, le paquet atterrit dans un dossier au nom haché
que rien ne peut retrouver.

## Le fichier de projet

`Plum.xcodeproj/project.pbxproj` est **généré** par
[`Scripts/generate_xcodeproj.py`](Scripts/generate_xcodeproj.py) à partir de
l'arborescence des sources. Après avoir ajouté ou supprimé un fichier :

```bash
python3 Scripts/generate_xcodeproj.py    # réécrit le projet
python3 Scripts/validate_pbxproj.py      # vérifie le graphe d'objets
```

Les identifiants d'objets sont dérivés du chemin de chaque fichier : deux
exécutions produisent le même fichier, ce qui évite les conflits de fusion
gratuits sur le projet Xcode. Ajouter une cible de tests revient à créer le
dossier et à la déclarer dans `TEST_TARGETS` ; un dossier absent est
simplement ignoré. Le validateur analyse le fichier et refuse les
références pendantes, les identifiants dupliqués et les fichiers compilés
mais absents du navigateur.
