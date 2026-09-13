# Plum

Plum est une application de rencontres pas sérieuses. Cette version est une
application **iOS native, écrite entièrement en Swift** : SwiftUI pour l'interface,
`async`/`await` et acteurs pour la concurrence, `URLSession` pour le réseau.
Aucune dépendance tierce.

## Ouvrir le projet

```bash
open Plum.xcodeproj
```

Le projet cible **iOS 17.0** et se construit avec Xcode 15 ou plus récent. Le
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

## Ce que fait l'application

- **Inscription et connexion** — jetons JWT, rafraîchissement automatique,
  stockage dans le trousseau, et une barrière 18+ vérifiée côté client.
- **Découverte** — un deck de cartes que l'on balaie à droite (j'aime), à
  gauche (non) ou vers le haut (coup de cœur), avec annulation du dernier
  passe, signalement et blocage depuis chaque carte.
- **Matchs** — l'écran de célébration, la rangée des matchs sans conversation
  entamée, puis la boîte de réception.
- **Messagerie** — historique paginé, envoi optimiste (la bulle apparaît
  avant la réponse du serveur), accusés de lecture, indicateur de saisie et
  reconnexion automatique du WebSocket.
- **Profil et réglages** — photos, bio, centres d'intérêt, critères de
  recherche, déconnexion et suppression de compte.

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

### L'API attendue

Le client parle à une API REST en `snake_case` avec des dates ISO 8601 :

```
POST   /auth/sign-up · /auth/sign-in · /auth/refresh · /auth/sign-out
GET    /me · /me/profile · /me/preferences
PATCH  /me/profile · /me/preferences · /me/photos/order
POST   /me/photos                       (multipart)
GET    /discovery/deck?limit=&cursor=
POST   /discovery/swipes · /discovery/rewind
GET    /matches · POST /matches/{id}/conversation
GET    /conversations · /conversations/{id}/messages
POST   /conversations/{id}/messages · /conversations/{id}/read
WS     /ws                              (message · read · typing · match)
```

## Tests

`⌘U` dans Xcode, ou :

```bash
xcodebuild test -scheme Plum -destination 'platform=iOS Simulator,name=iPhone 15'
```

La suite couvre le décodage du format de l'API, la construction des requêtes,
le rafraîchissement des jetons (y compris le 401 inattendu et l'échec qui
déconnecte), les seuils du geste de balayage, la validation du formulaire
d'inscription et l'envoi optimiste des messages.

## Intégration continue

`.github/workflows/ci.yml` fait deux choses à chaque PR :

- sur Ubuntu, il valide le graphe d'objets du projet et vérifie que le
  `pbxproj` committé correspond bien aux sources (la régénération ne doit
  produire aucun diff) ;
- sur macOS, il construit l'application et exécute la suite de tests sur un
  simulateur iPhone choisi à l'exécution par `Scripts/ci_test.sh`, plutôt
  que sur un nom de modèle codé en dur qu'une nouvelle image de runner
  casserait.

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
gratuits sur le projet Xcode. Le validateur analyse le fichier et refuse les
références pendantes, les identifiants dupliqués et les fichiers compilés
mais absents du navigateur.
