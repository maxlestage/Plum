/**
 * Aucune page ne doit glisser latéralement, à aucune largeur.
 *
 *     npm run build && node scripts/verifier-mise-en-page.mjs
 *
 * Écrit parce que j'ai introduit exactement ce défaut : trois pastilles de
 * thème ajoutées à l'en-tête, vérifiées à 400 px — où elles tiennent — et
 * débordant de vingt-quatre pixels à 320 px, où la page entière se mettait à
 * bouger de gauche à droite. 320 px n'est pas un cas d'école : c'est ce que
 * rapporte un iPhone dont l'affichage est réglé sur « Agrandi », et un
 * iPhone SE.
 *
 * D'où la largeur la plus étroite mesurée ici, 280 px : plus étroit que tout
 * téléphone réel, pour que le premier écran réel ait de la marge plutôt que
 * d'être le cas limite.
 *
 * Le débordement se mesure sur `scrollWidth > clientWidth`, et le coupable est
 * nommé : un rapport qui dit « ça déborde » sans dire de quoi oblige à
 * refaire la mesure à la main.
 */

import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const dist = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "dist");

const types = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css",
  ".js": "text/javascript",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".webmanifest": "application/manifest+json",
};

const serveur = http.createServer((requete, reponse) => {
  let cible = path.join(dist, decodeURIComponent(requete.url.split("?")[0]));
  if (fs.existsSync(cible) && fs.statSync(cible).isDirectory()) {
    cible = path.join(cible, "index.html");
  }
  if (!fs.existsSync(cible)) {
    reponse.writeHead(404).end("introuvable");
    return;
  }
  reponse.writeHead(200, {
    "Content-Type": types[path.extname(cible)] ?? "application/octet-stream",
  });
  reponse.end(fs.readFileSync(cible));
});

/** Les douze pages traduites, plus la racine. */
const chemins = [
  "/",
  "/fr/",
  "/en/",
  "/es/",
  "/fr/conditions/",
  "/fr/confidentialite/",
  "/fr/aide/",
  "/en/terms/",
  "/en/privacy/",
  "/en/help/",
  "/es/condiciones/",
  "/es/privacidad/",
  "/es/ayuda/",
];

/**
 * Portrait des téléphones courants, paysage des mêmes, et deux largeurs plus
 * étroites que tout appareil réel.
 */
const tailles = [
  [280, 600],
  [300, 640],
  [320, 568],
  [360, 640],
  [390, 844],
  [430, 932],
  [568, 320],
  [844, 390],
  [1280, 800],
];

await new Promise((resolu) => serveur.listen(0, resolu));
const base = `http://127.0.0.1:${serveur.address().port}`;

const navigateur = await chromium.launch(
  process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
);

const echecs = [];
let mesures = 0;

try {
  for (const [largeur, hauteur] of tailles) {
    const contexte = await navigateur.newContext({
      viewport: { width: largeur, height: hauteur },
    });
    for (const chemin of chemins) {
      const page = await contexte.newPage();
      await page.goto(base + chemin, { waitUntil: "networkidle" });

      const verdict = await page.evaluate((vue) => {
        const racine = document.documentElement;
        // Ce qui dépasse *à droite*. À gauche, un `left: -9999px` volontaire
        // — le lien d'évitement — ne crée pas de défilement et n'est pas un
        // défaut.
        const coupables = [];
        for (const element of document.querySelectorAll("*")) {
          const boite = element.getBoundingClientRect();
          if (boite.width === 0 && boite.height === 0) continue;
          if (boite.right <= vue + 0.5) continue;
          const classe =
            typeof element.className === "string" && element.className
              ? "." + element.className.trim().split(/\s+/).join(".")
              : "";
          coupables.push(
            `${element.tagName.toLowerCase()}${classe} (jusqu'à ${Math.round(boite.right)}px)`,
          );
        }
        return {
          defile: racine.scrollWidth > racine.clientWidth,
          scroll: racine.scrollWidth,
          client: racine.clientWidth,
          coupables: coupables.slice(0, 3),
        };
      }, largeur);

      mesures++;
      if (verdict.defile || verdict.coupables.length) {
        echecs.push(
          `${largeur}×${hauteur} ${chemin} — ${verdict.scroll}px de contenu pour ` +
            `${verdict.client}px d'écran\n        ${verdict.coupables.join("\n        ")}`,
        );
      }
      await page.close();
    }
    await contexte.close();
  }
} finally {
  await navigateur.close();
  serveur.close();
}

if (echecs.length) {
  console.error(`✗ ${echecs.length} page(s) glissent latéralement :\n`);
  for (const echec of echecs) console.error(`  - ${echec}`);
  console.error(
    "\n  Un en-tête qui ne tient pas se met à la ligne ; il ne rétrécit pas ses\n" +
      "  pastilles, dont la cible tactile est déjà au minimum de 44 px.",
  );
  process.exit(1);
}

console.log(
  `✓ aucune page ne glisse — ${mesures} mesures, de 280 à 1280 px de large`,
);
