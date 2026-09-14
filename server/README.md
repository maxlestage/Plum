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

## Vérifier un déploiement réel

`Scripts/parcours.sh` couvre l'API en HTTP. Deux vérifications de plus parlent
à un déploiement complet, socket et photos compris — ce qu'aucun test
d'intégration ne touche, puisqu'ils montent le serveur eux-mêmes :

```sh
cd Scripts && npm install     # une fois, pour `ws`
node verifier-photos.mjs      # envoi, réduction, service, suppression
node verifier-direct.mjs      # socket, évènements, garde-fous
```

Par défaut elles visent la production ; `PLUM_HOST` et `PLUM_TLS=0` les
pointent ailleurs. Elles créent leurs comptes et les effacent en partant,
même quand une vérification échoue.

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

L'image est construite dans GitHub Actions, jamais sur Heroku. Heroku ne
reçoit qu'une image finie, et la release ne prend que quelques secondes.

**Il faut désactiver les déploiements automatiques côté Heroku** (onglet
Deploy). Les deux chaînes ne peuvent pas coexister, et celle de Heroku échoue
de toute façon : la racine du dépôt ne contient aucun manifeste qu'il sache
détecter, le code étant dans `server/` et `web/`.

Le workflow pose lui-même le stack `container` par l'API avant de pousser,
parce que le registre de conteneurs l'exige et que ce réglage n'existe pas
dans le tableau de bord — c'est `heroku stack:set container`, donc une machine
avec un terminal. C'était la dernière étape qui imposait un ordinateur.

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
| `PUBLIC_BASE_URL` | L'adresse publique du déploiement, sans barre finale. Elle sert à écrire les adresses des photos, qui doivent être absolues : `AsyncImage` ne résout pas un chemin relatif. Sans elle, le serveur se rabat sur `http://127.0.0.1:{PORT}` — utile pour un `cargo run`, inutilisable depuis un téléphone. |

## Ce qui est fait, ce qui ne l'est pas

**Fait** : santé, inscription, connexion, rafraîchissement avec rotation,
déconnexion globale, `/me`, **suppression de compte**, et limitation de débit sur l'inscription et la
connexion — dix tentatives par quart d'heure et par adresse, comptées avant la
vérification du mot de passe pour qu'un limiteur ne révèle pas quelles
suppositions approchaient. Mots de passe en Argon2id, jetons de
rafraîchissement stockés en empreinte seulement, barrière 18+ vérifiée côté
serveur, et énumération des comptes fermée — une adresse inconnue et un
mauvais mot de passe répondent exactement la même chose.

**Trois quotas, et deux d'entre eux comptent l'adresse IP plutôt que l'email.**
Compter par adresse email n'arrête rien : un script qui en change à chaque
essai repart dans un seau neuf. L'inscription est donc aussi limitée à dix par
heure et par adresse IP, et l'envoi de photos à vingt par heure et par compte.
Sans ces deux-là, créer des comptes ne coûte rien et chaque compte peut déposer
six photos : le gigaoctet du plan Postgres se remplirait en un jour, et le
décodage de chaque image occuperait le seul dyno pendant ce temps.

L'adresse vient de la **dernière** valeur de `X-Forwarded-For`. Le routeur
Heroku ajoute l'adresse d'origine à droite de la liste, donc tout ce qui
précède a été envoyé par le client et se falsifie ; lire la première rendrait
la limite contournable en une ligne de `curl`, ce qu'un test vérifie.

**Une limite par adresse ne suffit pas, et c'est mesuré.** Depuis une machine
ordinaire, l'adresse de sortie tournait sur sept adresses d'un même bloc — donc
sept seaux, donc sept fois le quota. Changer d'adresse n'est pas une attaque,
c'est le fonctionnement normal de tout hébergeur, de tout VPN, de tout
mandataire. L'inscription est donc aussi comptée **par bloc** — /24 en IPv4,
/64 en IPv6 — avec un quota bien plus large, parce qu'un bloc peut abriter tout
un opérateur mobile derrière un NAT partagé.

**Et une limite qui ne dépend d'aucune identité.** Les quotas ci-dessus
comptent *qui* envoie ; on en change. Le dernier compte la ressource : au-delà
de 700 Mio de photos — lus par `pg_total_relation_size`, une lecture du
catalogue et non un parcours de table — l'envoi est refusé. Le reste de
l'application continue de fonctionner, ce qu'une base pleine ne permettrait
plus.

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
- **La distance quitte la base déjà arrondie**, aux paliers que le client
  affiche : moins d'un kilomètre, puis au kilomètre, puis par tranches de
  cinq. Ce n'est pas de la présentation, c'est la protection elle-même. Une
  distance exacte suffit à retrouver une adresse — il suffit de se placer à
  trois endroits, `PATCH /me/location` acceptant n'importe quelle position,
  de lire trois distances et de trianguler. Le curseur de pagination
  transporte la clé de tri jusqu'au client, donc le tri se fait sur la valeur
  arrondie lui aussi, sans quoi la précision fuirait par là. Un test vérifie
  les deux, et échoue bien lorsqu'on retire l'arrondi.
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

**Fait aussi** : la liste des matchs — `GET /matches`, `DELETE /matches/{id}`.

- Le curseur compare `(date, id)` comme celui du deck : deux personnes qui
  aiment en retour dans la même microseconde produiraient sinon un match sauté
  ou répété. La date est encodée à la microseconde, précision que garde la
  colonne `timestamptz`.
- Les profils sont chargés en une requête, pas une par match : trente
  allers-retours pour une liste qu'on ouvre à chaque lancement, c'est ce qui
  rend une application lente sans qu'on sache pourquoi.
- **Défaire un match ne supprime pas les verdicts.** Le deck exclut tout
  profil déjà jugé, donc les effacer ferait réapparaître la personne dont on
  vient de se séparer. Se défaire d'un match, c'est ne plus vouloir la voir.
- Défaire le match de quelqu'un d'autre répond « introuvable » plutôt
  qu'« interdit » : confirmer son existence renseignerait déjà.

**Fait aussi** : les conversations et les messages —
`POST /matches/{id}/conversation`, `GET /conversations`,
`GET`/`POST /conversations/{id}/messages`, `POST /conversations/{id}/read`.

- **Une conversation par match**, garantie par une clé unique. Deux appareils
  qui ouvrent l'écran en même temps créeraient sinon deux fils, et la
  discussion serait coupée en deux moitiés invisibles l'une à l'autre.
- **`client_id` empêche le double envoi.** Le client le tire au sort avant
  d'émettre ; une connexion coupée entre l'envoi et la réponse fait réessayer,
  et le renvoi retrouve le message déjà écrit au lieu d'en créer un second.
- **Le compte des non-lus est celui de qui demande** : jamais ses propres
  messages. De même, marquer comme lu ne touche que ce que l'autre a envoyé —
  marquer les siens reviendrait à répondre à sa place.
- **Un message vide n'est pas un message** : coupé, refusé, et une contrainte
  `CHECK` le redit à la base pour la prochaine route qui écrira ici.
- Une conversation à laquelle on n'appartient pas répond « introuvable »
  plutôt qu'« interdit ».

**Fait aussi** : le direct — `GET /ws`, hors de `/api/v1` parce qu'une mise à
niveau WebSocket n'est pas une requête versionnée.

- **Authentifié par l'en-tête `Authorization`**, comme le reste. Pas de jeton
  dans l'URL : une adresse se retrouve dans les journaux des serveurs
  mandataires et dans l'historique, un en-tête non.
- **Le serveur pousse `message`, `read`, `typing` et `match`** ; le client
  n'envoie que `typing`. Envoyer un message ou marquer un fil comme lu reste
  une requête : ces deux-là doivent pouvoir échouer franchement et rendre la
  ressource écrite, ce qu'une trame sans réponse ne sait pas faire.
- **Le direct n'est jamais la source de vérité.** Tout ce qui passe par là est
  déjà en base, et le client le relira au chargement suivant. C'est ce qui
  autorise à ne rien garantir sur la livraison — et à rester silencieux quand
  le destinataire n'est pas connecté.
- **« Untel écrit » est vérifié en base**, pas cru sur parole : sans ça,
  deviner un identifiant de conversation suffirait à faire clignoter l'écran
  de deux inconnus.
- **Un battement toutes les 30 secondes.** Le routeur Heroku ferme une
  connexion restée muette 55 secondes, et une conversation peut très bien
  rester calme plus longtemps.
- **Deux plafonds que le client ne peut pas contourner** : huit fenêtres par
  personne — au-delà, la plus ancienne est évincée plutôt que la nouvelle
  refusée, pour qu'un réseau qui saute n'enferme personne dehors — et une
  annonce de frappe par seconde et par connexion. Le client s'impose déjà
  trois secondes ; c'est une politesse qu'un client hostile n'a pas.
- Les tests de cette tranche montent un vrai serveur sur un vrai port : une
  poignée de main WebSocket ne survit pas à `tower::oneshot`.

**Fait aussi** : les photos — `POST /me/photos`, `PATCH /me/photos/order`,
`DELETE /me/photos/{id}`, et `GET /photos/{id}` pour les octets.

- **Les octets vivent dans Postgres**, pas dans un stockage objet. Ce n'est
  pas l'endroit habituel et c'est assumé : un stockage objet est un service
  payant de plus. Le plafond est **borné**, pas estimé : une photo stockée ne
  peut pas dépasser 200 Kio, parce que l'encodeur baisse la qualité jusqu'à y
  tenir. Une photo ordinaire fait 60 Kio et ne descend jamais sous la première
  qualité ; seules les images qui comprimeraient mal sont retouchées. Le
  gigaoctet du plan Postgres fait donc de l'ordre du millier de profils à six
  photos **même dans le pire cas**. Le jour où l'on s'en approche, seule la
  table change : l'adresse publique reste `/photos/{id}` et le client ne verra
  rien.

  Ce garde-fou a été ajouté après coup : la première version promettait
  150 Kio et en produisait 401 sur un motif à arêtes vives, ce que la mesure
  contre la production a montré. La promesse a été rendue vraie plutôt que
  réécrite à la baisse.
- **Rien n'est stocké tel qu'il arrive.** Le décodage sert de contrôle — un
  fichier qui n'est pas une image échoue là plutôt que d'être rangé puis
  servi — et le réencodage borne la taille, uniformise le format et **efface
  les métadonnées**. Ce dernier point est la raison principale : une photo
  sortie d'un téléphone porte ses coordonnées GPS dans son EXIF, et les
  publier sur une application de rencontres donnerait l'adresse de qui les
  publie.
- **Les adresses ne sont pas authentifiées**, parce qu'`AsyncImage` fait une
  requête nue. L'adresse est donc la clé : un UUID tiré au sort, cent
  vingt-deux bits, qui ne s'énumère pas. La contrepartie est écrite plutôt que
  découverte, ici et sur la page de confidentialité — qui détient un lien
  garde l'image, y compris après un match défait.
- **Le HEIC est refusé**, avec un message qui dit quoi faire. Le lire
  demanderait une bibliothèque C ; l'iPhone a déjà le décodeur, donc
  l'application convertit avant d'envoyer.
- **Une page de deck ne charge jamais les octets.** Construire vingt adresses
  ne demande que des identifiants ; charger la colonne d'octets ferait
  traverser une vingtaine de mégaoctets à chaque ouverture, pour n'en afficher
  aucun à ce moment-là.

**Variable à poser** : `PUBLIC_BASE_URL`, l'adresse publique du déploiement.
Les adresses des photos doivent être absolues — `AsyncImage` ne résout pas un
chemin relatif — et sans elle le serveur se rabat sur `http://127.0.0.1:{PORT}`,
ce qui donne des photos introuvables depuis un téléphone.

**Le site est rendu à la construction.** Les douze pages traduites partent
avec leur corps déjà en HTML ; le navigateur hydrate ensuite. Auparavant elles
portaient les bonnes métadonnées et un `<body>` vide — ce que les aperçus de
lien toléraient, puisqu'ils lisent les balises, mais pas les moteurs qui
n'exécutent pas le JavaScript. La racine reste une coquille : elle n'est pas
une page, seulement une redirection vers la langue détectée, donc il n'y a
rien à y rendre.

**Pas fait** : les notifications poussées, qui demandent un certificat APNs et
donc un compte développeur Apple — la seule chose ici que je ne peux pas poser
moi-même.

**À revoir avec plusieurs dynos** : deux choses.

- Les migrations tournent au démarrage. Avec une seule dyno c'est correct ;
  avec plusieurs, deux se marcheraient dessus et il faudra une phase
  `release`.
- **Le registre des sockets est en mémoire, donc par dyno.** Avec une seule,
  tout le monde est sur la même instance et c'est complet ; avec plusieurs,
  deux personnes tombées sur des dynos différents ne se verraient pas écrire.
  Il faudra alors relayer par Redis, déjà provisionné. C'est écrit ici plutôt
  que découvert plus tard : une limite nommée est une décision, une limite tue
  est un bug qui attend.
