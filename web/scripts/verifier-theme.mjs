/**
 * Éprouve la bascule de thème dans un vrai navigateur.
 *
 *     npm run build && node scripts/verifier-theme.mjs
 *
 * `Scripts/check_theme.py` tourne à chaque commit et vérifie ce qui est
 * statique : que les deux listes sombres sont identiques, que la clé de
 * stockage est la même des deux côtés, que les balises sont là. Il ne peut pas
 * vérifier ce qui compte vraiment — qu'un clic repeint la page.
 *
 * Ce script-là le peut, et ne tourne pas en intégration continue : il
 * demanderait Chromium sur la machine qui exécute des scripts Python. Il est
 * ici pour être relancé à la main après toute retouche du thème, et pour que
 * la mesure soit reproductible plutôt que racontée.
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
  reponse.writeHead(200, { "Content-Type": types[path.extname(cible)] ?? "application/octet-stream" });
  reponse.end(fs.readFileSync(cible));
});

const echecs = [];
function verifier(libelle, condition, detail = "") {
  console.log(`  ${condition ? "ok  " : "ÉCHEC"} ${libelle}${detail ? ` — ${detail}` : ""}`);
  if (!condition) echecs.push(libelle);
}

/**
 * Ouvre le menu de l'en-tête.
 *
 * Les pastilles y sont rangées depuis qu'il tient sur une ligne. Playwright
 * refuse de cliquer ce qui n'est pas visible, et c'est tant mieux : sans cette
 * étape, le script signalerait qu'il n'arrive pas à cliquer — ce qui serait
 * vrai pour un doigt aussi.
 */
const ouvrirMenu = (page) => page.click(".menu__bouton");

/** La couleur réellement peinte derrière le texte de la page. */
const fond = (page) =>
  page.evaluate(() => getComputedStyle(document.body).backgroundColor);

await new Promise((resolu) => serveur.listen(0, resolu));
const base = `http://127.0.0.1:${serveur.address().port}`;

// Le Chromium de l'environnement peut être d'une autre version que le
// paquet `playwright` installé, qui refuse alors de démarrer. `PW_CHROMIUM`
// désigne le binaire à employer ; sans lui, le comportement par défaut.
const navigateur = await chromium.launch(
  process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
);

try {
  // ---- 1. Sans choix : la page suit le système, dans les deux sens --------
  for (const systeme of ["light", "dark"]) {
    const contexte = await navigateur.newContext({ colorScheme: systeme, viewport: { width: 400, height: 800 } });
    const page = await contexte.newPage();
    await page.goto(`${base}/fr/`);
    const couleur = await fond(page);
    verifier(
      `système ${systeme} sans choix`,
      couleur === (systeme === "dark" ? "rgb(20, 12, 20)" : "rgb(255, 247, 244)"),
      couleur,
    );
    verifier(
      `système ${systeme} : « automatique » est la pastille pleine`,
      (await page.getAttribute('[aria-label="Thème automatique"]', "class")).includes("pastille--active"),
    );
    await contexte.close();
  }

  // ---- 2. Un choix explicite l'emporte sur le système ---------------------
  for (const [systeme, choix, attendu] of [
    ["dark", "Thème clair", "rgb(255, 247, 244)"],
    ["light", "Thème sombre", "rgb(20, 12, 20)"],
  ]) {
    const contexte = await navigateur.newContext({ colorScheme: systeme, viewport: { width: 400, height: 800 } });
    const page = await contexte.newPage();
    await page.goto(`${base}/fr/`);
    await ouvrirMenu(page);
    await page.click(`[aria-label="${choix}"]`);
    const couleur = await fond(page);
    verifier(`système ${systeme} + « ${choix} »`, couleur === attendu, couleur);

    // ---- 3. …et il survit au rechargement, sans éclair ------------------
    //
    // La mesure se prend sur `document.documentElement` au tout premier
    // événement du document : si l'attribut n'y est pas encore, le corps a
    // déjà été peint dans la mauvaise couleur.
    const page2 = await contexte.newPage();
    await page2.addInitScript(() => {
      document.addEventListener(
        "readystatechange",
        () => {
          window.__premierEtat ??= {
            etat: document.readyState,
            theme: document.documentElement.dataset.theme ?? null,
          };
        },
        { once: true },
      );
    });
    await page2.goto(`${base}/fr/`);
    const premier = await page2.evaluate(() => window.__premierEtat);
    verifier(
      `« ${choix} » posé avant le premier rendu`,
      premier?.theme === (choix === "Thème clair" ? "light" : "dark"),
      JSON.stringify(premier),
    );
    verifier(`« ${choix} » survit au rechargement`, (await fond(page2)) === attendu);

    // ---- 4. Revenir à « automatique » rend la main au système -----------
    await ouvrirMenu(page2);
    await page2.click('[aria-label="Thème automatique"]');
    const rendu = await fond(page2);
    verifier(
      `retour à automatique sous un système ${systeme}`,
      rendu === (systeme === "dark" ? "rgb(20, 12, 20)" : "rgb(255, 247, 244)"),
      rendu,
    );
    verifier(
      "automatique n'écrit rien dans le stockage",
      (await page2.evaluate(() => localStorage.getItem("plum.theme"))) === null,
    );

    await contexte.close();
  }

  // ---- 5. La barre du navigateur suit, elle aussi ------------------------
  {
    const contexte = await navigateur.newContext({ colorScheme: "light", viewport: { width: 400, height: 800 } });
    const page = await contexte.newPage();
    await page.goto(`${base}/fr/`);
    await ouvrirMenu(page);
    await page.click('[aria-label="Thème sombre"]');
    const teintes = await page.evaluate(() =>
      [...document.querySelectorAll('meta[name="theme-color"]')].map((m) => [
        m.content,
        m.getAttribute("media"),
      ]),
    );
    verifier(
      "theme-color bascule et perd son media",
      teintes.every(([contenu, media]) => contenu.toLowerCase() === "#40183a" && media === null),
      JSON.stringify(teintes),
    );
    await contexte.close();
  }

  // ---- 6. Aucune erreur d'hydratation ------------------------------------
  {
    const contexte = await navigateur.newContext({ colorScheme: "dark", viewport: { width: 400, height: 800 } });
    const page = await contexte.newPage();
    const plaintes = [];
    page.on("console", (message) => {
      if (message.type() === "error" || message.type() === "warning") plaintes.push(message.text());
    });
    page.on("pageerror", (erreur) => plaintes.push(String(erreur)));
    await page.goto(`${base}/fr/`);
    await page.waitForTimeout(400);
    verifier("la console reste muette", plaintes.length === 0, plaintes.join(" | ").slice(0, 300));

    // La mise en page, elle, se mesure ailleurs : `verifier-mise-en-page.mjs`
    // la prend à neuf largeurs, de 280 à 1280 px. Il y avait ici une
    // assertion « l'en-tête tient sur une ligne à 400 px » — vraie, verte, et
    // c'est elle qui m'a fait croire l'affaire réglée pendant que l'en-tête
    // débordait à 320.

    await page.screenshot({ path: process.env.CAPTURE ?? "/tmp/entete-sombre.png", clip: { x: 0, y: 0, width: 400, height: 64 } });
    await contexte.close();
  }
} finally {
  await navigateur.close();
  serveur.close();
}

console.log(echecs.length ? `\n✗ ${echecs.length} vérification(s) en échec` : "\n✓ la bascule de thème tient");
process.exit(echecs.length ? 1 : 0);
