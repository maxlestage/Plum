# Site de présentation

React + TypeScript + Vite. Servi par le même dyno Heroku que l'API : à la
racine pour le site, `/api/v1` pour l'API.

## Pourquoi le même dyno

Un second dyno coûterait plus cher par mois que le premier, pour quelques
fichiers statiques. Et le séparer mettrait le site, l'API et les pages légales
que l'App Store réclame sur trois hôtes différents, à maintenir vivants
séparément.

Le serveur Rust sert `web/dist` en repli de tout ce que l'API ne réclame pas,
avec retour sur `index.html` — sans quoi `/conditions` ne fonctionnerait que
depuis un lien interne, jamais tapé dans la barre d'adresse. Un test
d'intégration vérifie que ni l'un ni l'autre ne masque l'autre.

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

`/conditions` et `/confidentialite` décrivent fidèlement ce que fait
l'application — elles ont été écrites depuis le code. Elles portent un bandeau
qui dit ce qu'elles sont : **des brouillons non relus par un juriste**. Ce
n'est pas une précaution de forme. Les critères de recherche d'une application
de rencontres permettent d'inférer une orientation sexuelle, que le RGPD range
parmi les catégories particulières de l'article 9.
