/**
 * Combien de temps la page est lisible mais sourde.
 *
 * Le pré-rendu livre le texte tout de suite ; les pastilles de thème ne
 * répondent qu'une fois le wasm arrivé et l'arbre hydraté. Cet écart est le
 * coût du passage à Yew pour quelqu'un qui tape vite.
 *
 * Chaque mesure à froid part d'un contexte neuf — le cache d'un contexte
 * Playwright lui est propre, donc trois tours dans le même contexte ne font
 * qu'une visite froide suivie de deux chaudes. La première version de ce
 * script en prenait la médiane, ce qui noyait précisément ce qu'il mesurait.
 */
import { chromium } from "playwright";
import { attendreHydratation, optionsChromium, origine } from "./origine.mjs";

const { base, distante, fermer } = await origine();
console.log(`  (${distante ? "origine déployée" : "dist local"} : ${base})`);
const navigateur = await chromium.launch(optionsChromium());
const vue = { viewport: { width: 390, height: 844 } };

const unTour = async (contexte) => {
  const page = await contexte.newPage();
  const depart = Date.now();
  await page.goto(`${base}/fr/`, { waitUntil: "commit" });
  await page.waitForSelector("main h1", { state: "attached" }); // le texte pré-rendu
  const lisible = Date.now() - depart;
  await attendreHydratation(page);
  const interactif = Date.now() - depart;
  await page.close();
  return [lisible, interactif];
};

const rapporter = (libelle, mesures) => {
  const col = (i) => mesures.map((m) => m[i]).sort((a, b) => a - b);
  const [l, inter] = [col(0), col(1)];
  const mediane = (t) => t[(t.length - 1) >> 1];
  console.log(
    `  ${libelle.padEnd(12)} lisible ${String(mediane(l)).padStart(5)} ms   ` +
      `interactif ${String(mediane(inter)).padStart(5)} ms   ` +
      `sourde ${String(mediane(inter) - mediane(l)).padStart(4)} ms   ` +
      `(interactif : ${inter.join(", ")})`,
  );
};

try {
  const froides = [];
  for (let tour = 0; tour < 3; tour++) {
    const contexte = await navigateur.newContext(vue); // cache vierge à chaque fois
    froides.push(await unTour(contexte));
    await contexte.close();
  }
  rapporter("cache froid", froides);

  const chaud = await navigateur.newContext(vue);
  await unTour(chaud); // la visite qui remplit le cache, écartée
  const chaudes = [];
  for (let tour = 0; tour < 3; tour++) chaudes.push(await unTour(chaud));
  rapporter("cache chaud", chaudes);
  await chaud.close();
} finally {
  await navigateur.close();
  fermer();
}
