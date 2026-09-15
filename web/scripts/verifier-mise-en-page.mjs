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
 * D'où la largeur la plus étroite mesurée ici, 180 px. Elle a d'abord été de
 * 280 px — « plus étroit que tout téléphone » — et c'était encore trop haut :
 * la largeur qui compte n'est pas celle de l'appareil mais la largeur
 * *effective*, et à 200 % de zoom, ce que règle couramment quelqu'un qui voit
 * mal, un téléphone de 390 px n'en offre plus que 195.
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
  [180, 600],
  [220, 600],
  [260, 600],
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

/**
 * Ce qui, sur cette page, sort de l'écran.
 *
 * Une seule fonction pour les deux passes — au repos et sous les gestes — et
 * ce n'est pas du rangement. Elles avaient deux définitions du débordement :
 * la première nommait les coupables, la seconde se contentait de
 * `scrollWidth`. Or un panneau de menu ne s'ouvre que pendant un geste, et il
 * sortait par la *gauche*, ce que `scrollWidth` ne voit pas. Le défaut ne
 * tombait donc dans aucune des deux.
 */
async function mesurer(page, vue) {
  return page.evaluate((vue) => {
    const racine = document.documentElement;
    const coupables = [];
    for (const element of document.querySelectorAll("*")) {
      /*
       * Mesuré, et surprenant : un `<details>` fermé garde à ses enfants une
       * boîte de mise en page pleine et entière — Chromium leur pose
       * `content-visibility: hidden`, qui cesse de les peindre sans cesser de
       * les disposer. `getBoundingClientRect` rend donc 208 × 156 pour un
       * panneau que personne ne voit.
       *
       * `checkVisibility()` est la seule question qui vaille ici : est-ce que
       * quelqu'un le voit.
       */
      if (element.checkVisibility && !element.checkVisibility()) continue;
      const boite = element.getBoundingClientRect();
      if (boite.width === 0 && boite.height === 0) continue;

      /*
       * Les deux bords, et le critère est le chevauchement, pas le côté.
       *
       * À droite c'est évident : ça crée du défilement. À gauche, non — un
       * élément coupé par le bord gauche n'allonge pas la page, donc
       * `scrollWidth` n'en dit rien. J'avais écarté ce cas en le justifiant
       * (« un `left: -9999px` volontaire n'est pas un défaut ») et un panneau
       * de menu rogné de moitié est passé au travers.
       *
       * Un élément *à cheval* sur un bord est coupé ; un élément entièrement
       * au-delà est rangé. Le lien d'évitement parqué à -9999 px est
       * entièrement dehors — son bord droit vaut -9807 — donc il ne chevauche
       * rien.
       */
      const coupeAGauche = boite.left < -0.5 && boite.right > 0.5;
      if (boite.right <= vue + 0.5 && !coupeAGauche) continue;

      const classe =
        typeof element.className === "string" && element.className
          ? "." + element.className.trim().split(/\s+/).join(".")
          : "";
      coupables.push(
        `${element.tagName.toLowerCase()}${classe} (${Math.round(boite.left)} → ` +
          `${Math.round(boite.right)}px)`,
      );
    }
    return {
      defile: racine.scrollWidth > racine.clientWidth || coupables.length > 0,
      scroll: racine.scrollWidth,
      client: racine.clientWidth,
      coupables: coupables.slice(0, 3),
    };
  }, vue);
}

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

      const verdict = await mesurer(page, largeur);

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

// ---------------------------------------------------------------------------
// Et pendant qu'on s'en sert.
//
// Tout ce qui précède mesure une page au repos. Une page peut tenir au
// chargement et déborder au premier geste : le lien d'évitement, caché à
// `left: -9999px`, revient à `left: 0` dès qu'on l'atteint au clavier, et il
// est plus large que certains écrans. Les pastilles de thème, elles, changent
// des couleurs — en principe rien de géométrique, ce qui est exactement le
// genre de « en principe » qui se vérifie en trois secondes.
// ---------------------------------------------------------------------------

const navigateur2 = await chromium.launch(
  process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
);
const serveur2 = http.createServer(serveur.listeners("request")[0]);
await new Promise((ok) => serveur2.listen(0, ok));
const base2 = `http://127.0.0.1:${serveur2.address().port}`;

try {
  for (const [largeur, hauteur] of [
    [180, 600],
    [320, 568],
    [390, 844],
  ]) {
    const contexte = await navigateur2.newContext({
      viewport: { width: largeur, height: hauteur },
    });
    /*
     * Chaque geste part d'une page fraîche.
     *
     * Enchaînés sur la même page, ils se contaminent : le lien d'évitement
     * gardé au focus recouvre l'en-tête — légitimement, c'est un panneau
     * posé par-dessus et il s'efface dès que le focus bouge — et les clics
     * suivants échouaient sur lui. Le rapport accusait alors le menu, qui
     * n'y était pour rien. Personne n'enchaîne ces six gestes sans jamais
     * rien relâcher.
     */
    const gestes = [
      ["le lien d'évitement reçoit le focus", async (page) => {
        await page.keyboard.press("Tab");
      }],
      // Le panneau ouvert est un état à part entière : c'est le plus large
      // morceau d'interface du site, posé contre le bord droit, et donc le
      // premier candidat à sortir de l'écran.
      ["on ouvre le menu", async (page) => {
        await page.click(".menu__bouton");
      }],
      ["on choisit le thème sombre", async (page) => {
        await page.click(".menu__bouton");
        await page.click('[aria-label="Thème sombre"]');
      }],
      ["on choisit le thème clair", async (page) => {
        await page.click(".menu__bouton");
        await page.click('[aria-label="Thème clair"]');
      }],
      ["on referme le menu à l'Échap", async (page) => {
        await page.click(".menu__bouton");
        await page.keyboard.press("Escape");
      }],
      ["on descend au bas de la page", async (page) => {
        await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
      }],
      ["on change de langue", async (page) => {
        await page.click(".menu__bouton");
        await page.click('[aria-label="English"]');
        await page.waitForURL(/\/en\//);
      }],
    ];

    for (const [libelle, geste] of gestes) {
      mesures++;
      const page = await contexte.newPage();
      // Cinq secondes suffisent à cliquer un bouton visible ; les trente par
      // défaut ne servent qu'à rallonger l'attente quand c'est déjà cassé.
      page.setDefaultTimeout(5000);
      await page.goto(`${base2}/fr/`, { waitUntil: "networkidle" });

      try {
        await geste(page);
      } catch (erreur) {
        // Un geste qui échoue *est* un défaut de mise en page, et c'est même
        // le pire : quelque chose recouvre le bouton. Sans ce filet, le script
        // mourait sur « Timeout 30000ms exceeded » — vrai, rouge, et muet sur
        // la cause. Un contrôle dont l'échec n'explique rien est un contrôle
        // qu'on apprend à ignorer.
        echecs.push(
          `${largeur}×${hauteur} — impossible de faire « ${libelle} » : ` +
            `${String(erreur).split("\n")[0]}\n        (un élément en recouvre ` +
            `probablement un autre)`,
        );
        await page.close();
        continue;
      }

      await page.waitForTimeout(120);
      const verdict = await mesurer(page, largeur);
      if (verdict.defile) {
        echecs.push(
          `${largeur}×${hauteur} après « ${libelle} » — ${verdict.scroll}px de ` +
            `contenu pour ${verdict.client}px d'écran\n        ` +
            verdict.coupables.join("\n        "),
        );
      }
      await page.close();
    }

    await contexte.close();
  }
} finally {
  await navigateur2.close();
  serveur2.close();
}

if (echecs.length) {
  console.error(`✗ ${echecs.length} défaut(s) de mise en page :\n`);
  for (const echec of echecs) console.error(`  - ${echec}`);
  console.error(
    "\n  Un en-tête qui ne tient pas se met à la ligne ; il ne rétrécit pas ses\n" +
      "  pastilles, dont la cible tactile est déjà au minimum de 44 px.",
  );
  process.exit(1);
}

console.log(
  `✓ aucune page ne glisse — ${mesures} mesures, de 180 à 1280 px de large,` +
    " au repos et sous sept gestes",
);
