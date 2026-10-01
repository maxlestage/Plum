# Site de présentation

**Rust + Yew, compilé en WebAssembly par Trunk.** Servi par le même dyno
Heroku que l'API : à la racine pour le site, `/api/v1` pour l'API.

```bash
rustup target add wasm32-unknown-unknown
trunk build --release --features hydration      # le wasm et la coquille
cargo run --release --features ssr --bin prerender   # les treize pages
npm run verifier                                # les deux gardes du navigateur
```

## Ce que le passage de React à Yew a coûté, et ce qu'il n'a pas changé

Il a remplacé 1600 lignes de TypeScript par un crate Rust de forme identique.
Ce qui n'a pas bougé : les treize pages sont toujours **pré-rendues en HTML**
au moment de la construction, donc lisibles sans que rien ne s'exécute — c'est
ce que lit un robot, et ce que lit la revue de l'App Store sur les pages
légales. Le même arbre de composants sert au pré-rendu et à l'hydratation,
puisque c'est le même crate ; les deux implémentations séparées qu'il y avait
avant, une pour le serveur et une pour le client, ne garantissaient pas ça.

Ce qu'il a coûté, mesuré : **le transfert passe de 67 à 196 Ko gzippés**, soit
près de trois fois. Le wasm pèse 181 Ko gzippés à lui seul, là où tout le
bundle React tenait en 66. Sur un téléphone en données mobiles, ça se sent. Le
texte, lui, arrive avant le wasm — c'est l'intérêt du pré-rendu — donc la page
se lit tout de suite et devient interactive plus tard qu'avant.

Combien plus tard : `scripts/mesurer-hydratation.mjs` chronomètre l'écart entre
les deux. Contre la production, sur une première visite, **la page se lit à
350 ms et ne répond qu'à 1 150 ms** — huit cents millisecondes pendant
lesquelles le texte est là, les liens fonctionnent, et une pastille de thème ne
fait rien. Les visites suivantes trouvent le wasm en cache et ramènent l'écart
à une vingtaine de millisecondes. Ces chiffres viennent d'un centre de données ;
sur un téléphone en données mobiles la première visite est pire, pas meilleure.

Contre le `dist` local, l'écart est de vingt millisecondes **dès la première
visite** — le wasm vient de `127.0.0.1`. C'est pour ça que les gardes du
navigateur ne voyaient pas ce délai, et qu'ils cliquaient une pastille que
personne n'écoutait encore sans jamais échouer : il a fallu les pointer sur
l'origine déployée pour que le défaut sorte.

Les traductions ont été extraites des modules TypeScript **en les évaluant**,
pas en les recopiant : les trois fichiers JSON de `src/i18n/` sont le texte
d'origine au mot près. Et les deux gardes du navigateur — 177 mesures de mise
en page, 16 contrôles de thème — lisent le `dist` construit sans savoir ce qui
l'a produit. C'est ce qui a permis de prouver que le remplacement n'avait rien
changé au comportement, et c'est ce qui a attrapé le seul vrai défaut du
portage : `Renderer::new()` hydrate `<body>` alors que le corps pré-rendu vit
dans `#root`, ce qui donnait une page qui s'affichait bien et ne répondait à
rien.

## Pourquoi le même dyno

Un second dyno coûterait plus cher par mois que le premier, pour quelques
fichiers statiques. Et le séparer mettrait le site, l'API et les pages légales
que l'App Store réclame sur trois hôtes différents, à maintenir vivants
séparément.

Le serveur Rust sert `web/dist` en repli de tout ce que l'API ne réclame pas,
avec retour sur `index.html` — sans quoi `/fr/conditions` ne fonctionnerait que
depuis un lien interne, jamais tapé dans la barre d'adresse. Un test
d'intégration vérifie que ni l'un ni l'autre ne masque l'autre.

## Trois langues

Français, anglais, espagnol. La langue est **dans l'URL** (`/fr`, `/en`, `/es`)
et non dans un état interne : chaque version a son adresse, partageable,
indexable et stable après rechargement, sans rien stocker sur la machine du
visiteur.

Les slugs sont traduits eux aussi — `/es/privacidad`, pas `/es/privacy` : un
lecteur hispanophone n'a pas à lire l'anglais pour savoir où mène un lien. Le
code route sur une clé de page, donc les slugs peuvent changer sans toucher un
composant.

La racine `/` détecte la langue depuis `navigator.languages`, sur le sous-tag
principal seulement — `es-419` et `es-MX` arrivent sur l'espagnol plutôt que de
retomber par défaut faute de région connue. **Le repli est le français**, parce
que l'application, elle, est en français : envoyer un visiteur non reconnu vers
l'anglais lui promettrait un produit qui n'existe pas.

Pour la même raison, les pages anglaise et espagnole disent explicitement que
l'application n'est pour l'instant qu'en français. Traduire le site sans le dire
serait un appât.

### Des gabarits statiques par langue

Le second binaire du crate, `src/bin/prerender.rs`, écrit un `index.html`
par langue **et par page** — douze en tout, plus la racine — avec le bon
`lang`, le bon titre, la bonne description et les liens `hreflang` déjà dedans.
Il rend le même arbre de composants que le navigateur, par `ServerRenderer`,
et c'est pour ça qu'il est dans le même crate : deux implémentations séparées
ne resteraient pas d'accord.

Sans ça, le site reste une application monopage : le HTML livré porte un seul
titre, en français, et l'application le corrige une fois le wasm chargé. C'est
suffisant pour une personne et inutile pour les robots qui fabriquent les
aperçus de liens — aucun n'exécute de JavaScript. Partager `/es` dans WhatsApp
ou iMessage aurait affiché le titre français.

`ServeDir` sert `/es/privacidad/index.html` tout seul pour `/es/privacidad/`,
et l'application prend le relais ensuite.

**Toutes les adresses portent une barre finale.** `ServeDir` répond à une URL
de répertoire sans barre par une redirection 307 vers celle qui en a une :
déclarer la forme sans barre dans les `hreflang` ferait pointer chaque
traduction vers une adresse qui redirige avant de servir, et la barre
d'adresse contredirait le `canonical` après un rechargement. Un `canonical`
est posé dans chaque gabarit, sur cette même forme.

Corollaire attrapé par les tests : le composant qui pose les `hreflang` à
l'exécution doit **remplacer** ceux du gabarit, pas s'ajouter à eux, sinon
chaque page en porte deux jeux contradictoires.

### Pas de bibliothèque i18n

Des structures Rust (`src/i18n/mod.rs`) désérialisées depuis `fr.json`,
`en.json` et `es.json` par `serde`, plutôt qu'une bibliothèque. Pour trois
langues et quatre pages, ça achète la seule garantie qui compte : la forme est
exacte, donc **une clé manquante en espagnol casse la compilation**. Les étapes
et les principes sont même des tableaux de taille fixe — `[Entry; 3]`,
`[Entry; 6]` — donc en ajouter un en français sans le traduire ne compile pas
non plus. Une recherche par chaîne de caractères, elle, afficherait un blanc
que personne ne verrait avant un visiteur.

Les trois JSON sont lus par `include_str!`, donc ils entrent dans le binaire :
pas de fichier à déployer à côté, et pas de chemin à trouver à l'exécution.

## Développer

```bash
trunk serve --features hydration    # rechargement à chaud sur :8080
cargo test                          # les tables de langues et de slugs
cargo clippy --all-targets
```

`trunk serve` sert la coquille de `index.html`, pas les pages pré-rendues :
l'application route elle-même, donc `/es/privacidad/` fonctionne, mais le
titre de départ est celui de la coquille. Pour éprouver le pré-rendu il faut
la construction complète, celle de l'encadré du haut.

Node ne reste ici que pour les deux gardes du navigateur. `npm ci` les
installe ; rien du site ne passe par lui.

### Vérifier un déploiement avec ce qui vérifie une construction

Les deux gardes lisent par défaut le `dist` local, servi par un serveur
éphémère. `PLUM_BASE` les envoie sur une origine déjà déployée :

```bash
PLUM_BASE=https://plum-a5f3c7189761.herokuapp.com npm run verifier
```

C'est le même jeu d'assertions — 177 mesures de mise en page, 16 contrôles de
thème — et c'est le point : un déploiement se vérifie avec ce qui vérifie une
construction, pas avec un coup d'œil.

Deux variables aident selon la machine. `PW_CHROMIUM` désigne le binaire à
piloter, quand celui de l'environnement est d'une autre version que le paquet
`playwright`. `PLUM_SPKI` nomme des autorités de certification à accepter bien
qu'elles soient inconnues du magasin — une liste d'empreintes SHA-256 de clé
publique — pour interroger une origine HTTPS derrière un proxy qui inspecte le
TLS. Elle nomme les autorités qu'on accepte ; elle ne désactive pas la
vérification, donc un certificat expiré fait toujours échouer la navigation.

C'est en pointant ces gardes sur l'origine déployée qu'un défaut *du garde* est
apparu : ils cliquaient une pastille que le pré-rendu a déjà peinte mais que
personne n'écoutait encore, parce que le wasm n'était pas arrivé. En local il
arrive en quelques millisecondes et le défaut ne se voyait pas ; six
vérifications sur seize tombaient contre Heroku. D'où `attendreHydratation`,
qui guette la marque que seul l'arbre hydraté laisse dans la page.

## La palette

Les couleurs de `src/theme.css` sont celles de `PlumTheme.swift`, au chiffre
hexadécimal près, et `Scripts/check_palette.py` le vérifie. Un site qui dérive
du produit qu'il présente a l'air d'une contrefaçon.

## Les pages légales

`/fr/conditions`, `/en/terms`, `/es/condiciones` et leurs équivalents de
confidentialité décrivent fidèlement ce que fait l'application — elles ont été écrites depuis le code. Elles portent un bandeau
qui dit ce qu'elles sont : **des brouillons non relus par un juriste**. Ce
n'est pas une précaution de forme. Les critères de recherche d'une application
de rencontres permettent d'inférer une orientation sexuelle, que le RGPD range
parmi les catégories particulières de l'article 9.

Le bandeau est traduit lui aussi : un brouillon traduit reste un brouillon, et
c'est justement dans une autre langue qu'on serait tenté de le prendre pour un
document officiel.
