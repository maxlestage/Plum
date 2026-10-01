/**
 * L'application démarre-t-elle, et sur quelle adresse ?
 *
 *     npm run verifier
 *     PLUM_BASE=https://… node scripts/verifier-demarrage.mjs
 *
 * Écrit parce que la racine `/` rendait une page blanche en production, et que
 * rien ne le disait. `main.rs` hydratait sans condition ; or `/` n'est pas
 * pré-rendue — l'application y choisit une langue avant de rendre quoi que ce
 * soit — donc Yew hydratait un conteneur vide, paniquait sur « expected
 * Component opening tag, found EOF », et le wasm mourait. L'adresse nue du
 * site, celle que quelqu'un tape en premier, ne montrait rien.
 *
 * Les trois contrôles qui existaient étaient verts dessus : le garde de mise
 * en page visitait `/` mais n'y mesurait que le débordement, vrai sur une page
 * blanche ; le garde de thème ne chargeait que `/fr/` ; et l'oracle du portage
 * comparait le texte visible, vide avant comme après, donc identique.
 *
 * Ce garde-ci regarde la seule chose qui manquait : que le conteneur se
 * remplisse, et que la console se taise. Un panique de wasm s'y voit, où qu'il
 * survienne.
 */

import { chromium } from "playwright";

import { optionsChromium, origine } from "./origine.mjs";

const { base, distante, fermer } = await origine();
console.log(`  (${distante ? "origine déployée" : "dist local"} : ${base})`);

const navigateur = await chromium.launch(optionsChromium());
const echecs = [];
const dire = (ok, texte, detail = "") => {
  if (!ok) echecs.push(texte);
  console.log(`  ${ok ? "ok  " : "ÉCHEC"} ${texte}${detail ? ` — ${detail}` : ""}`);
};

/**
 * Charge une adresse et rend ce que l'application en a fait.
 *
 * L'attente porte sur un `h1` dans `main` : c'est ce que toute page du site
 * finit par contenir, et ce qu'une page morte ne contiendra jamais. Son
 * expiration est un échec, pas une exception — un garde doit rapporter, pas
 * s'interrompre au premier défaut et taire les suivants.
 */
async function ouvrir(chemin, langueDuNavigateur) {
  const contexte = await navigateur.newContext({
    viewport: { width: 390, height: 844 },
    locale: langueDuNavigateur,
  });
  const page = await contexte.newPage();
  const plaintes = [];
  page.on("console", (message) => {
    if (message.type() === "error" || message.type() === "warning") {
      plaintes.push(message.text().split("\n")[0].slice(0, 180));
    }
  });
  page.on("pageerror", (erreur) => plaintes.push(String(erreur).split("\n")[0].slice(0, 180)));

  await page.goto(base + chemin, { waitUntil: "load" });
  await page.waitForSelector("main h1", { timeout: 20000 }).catch(() => {});
  const vu = await page.evaluate(() => ({
    contenu: document.getElementById("root")?.innerHTML.length ?? -1,
    texte: (document.body.innerText || "").trim().length,
    chemin: location.pathname,
    langue: document.documentElement.lang,
    titre: document.title,
  }));
  await contexte.close();
  return { ...vu, plaintes };
}

try {
  // ---- 1. La racine choisit une langue et rend la page --------------------
  //
  // Les trois langues, parce que c'est `navigator.languages` qui décide, et
  // qu'un repli muet sur le français passerait inaperçu sur un seul essai.
  for (const [locale, attendu] of [
    ["fr-FR", "/fr/"],
    ["en-GB", "/en/"],
    ["es-ES", "/es/"],
    // Une langue que le site ne parle pas : le repli est le français.
    ["de-DE", "/fr/"],
  ]) {
    const vu = await ouvrir("/", locale);
    dire(
      vu.chemin === attendu && vu.texte > 1500,
      `/ sous ${locale} mène à ${attendu}`,
      `${vu.chemin}, ${vu.texte} caractères, « ${vu.titre} »`,
    );
    dire(vu.plaintes.length === 0, `/ sous ${locale} : la console se tait`, vu.plaintes.join(" | "));
  }

  // ---- 2. Les pages pré-rendues s'hydratent sans se plaindre --------------
  for (const chemin of ["/fr/", "/en/terms/", "/es/privacidad/", "/fr/aide/"]) {
    const vu = await ouvrir(chemin, "fr-FR");
    dire(
      vu.chemin === chemin && vu.texte > 400,
      `${chemin} se rend`,
      `${vu.texte} caractères, lang=${vu.langue}, « ${vu.titre} »`,
    );
    dire(vu.plaintes.length === 0, `${chemin} : la console se tait`, vu.plaintes.join(" | "));
  }
} finally {
  await navigateur.close();
  fermer();
}

console.log(
  echecs.length
    ? `\n✗ ${echecs.length} vérification(s) en échec`
    : "\n✓ l'application démarre sur chaque adresse, racine comprise",
);
process.exit(echecs.length ? 1 : 0);
