# Site de présentation

React + TypeScript + Vite. Servi par le même dyno Heroku que l'API : à la
racine pour le site, `/api/v1` pour l'API.

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

### Pas de bibliothèque i18n

Un dictionnaire typé (`src/i18n/types.ts`) plutôt que `react-i18next`. Pour
trois langues et quatre pages, ça achète la seule garantie qui compte : `Copy`
est une forme exacte, donc **une clé manquante en espagnol casse le build**. Une
recherche par chaîne de caractères, elle, afficherait un blanc que personne ne
verrait avant un visiteur. Comme `npm run build` lance `tsc -b`, la CI le
vérifie à chaque image construite.

## Développer

```bash
npm install
npm run dev      # proxifie /api vers http://127.0.0.1:8080
npm run build    # tsc -b puis vite build
npm run lint     # tsc --noEmit
```

TypeScript est en `strict`, avec `noUncheckedIndexedAccess` et
`noUnusedLocals`. Le build échoue sur une erreur de types, donc l'image aussi.

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
