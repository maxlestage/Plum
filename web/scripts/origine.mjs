/**
 * D'où les gardes du navigateur lisent le site.
 *
 * Par défaut : un serveur éphémère sur le `dist` construit. Les gardes ne
 * savent rien de ce qui l'a produit, et c'est cette ignorance qui a permis de
 * prouver que le passage de React à Yew n'avait rien changé au comportement.
 *
 * Avec `PLUM_BASE`, elles interrogent une origine déjà déployée :
 *
 *     PLUM_BASE=https://plum-a5f3c7189761.herokuapp.com node scripts/verifier-theme.mjs
 *
 * C'est le même jeu d'assertions, et c'est le point : un déploiement se
 * vérifie avec ce qui vérifie une construction, pas avec un coup d'œil. Le
 * site est pré-rendu et sert du statique — aucune assertion n'a de raison de
 * dépendre de l'endroit d'où le fichier vient.
 */

import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const dist = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "dist");

const types = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css",
  ".js": "text/javascript",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".webmanifest": "application/manifest+json",
  // Le site est compilé en WebAssembly : sans ce type, le navigateur refuse
  // `instantiateStreaming`, se rabat sur un chargement plus lent, et le dit
  // dans la console — ce que la vérification « la console reste muette »
  // relève, à juste titre.
  ".wasm": "application/wasm",
};

const repondre = (requete, reponse) => {
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
};

/**
 * Une origine à interroger, et de quoi la refermer.
 *
 * Les gardes en ouvrent une ou deux — la mise en page en veut deux, pour deux
 * navigateurs — et appellent `fermer()` dans un `finally`. Contre une origine
 * distante, `fermer()` n'a rien à faire : il n'y a rien à nous.
 */
export async function origine() {
  const distante = process.env.PLUM_BASE?.replace(/\/$/, "");
  if (distante) {
    return { base: distante, distante: true, fermer: () => {} };
  }

  const serveur = http.createServer(repondre);
  await new Promise((resolu) => serveur.listen(0, resolu));
  return {
    base: `http://127.0.0.1:${serveur.address().port}`,
    distante: false,
    fermer: () => serveur.close(),
  };
}

/** Le Chromium à piloter.
 *
 * Celui de l'environnement peut être d'une autre version que le paquet
 * `playwright` installé, qui refuse alors de démarrer. `PW_CHROMIUM` désigne
 * le binaire à employer ; sans lui, le comportement par défaut.
 *
 * `PLUM_SPKI` nomme des autorités de certification à accepter bien qu'elles
 * soient inconnues du magasin : une liste d'empreintes SHA-256 de clé
 * publique, séparées par des virgules. Elle sert à vérifier une origine
 * HTTPS depuis un environnement qui inspecte le TLS — un bac à sable
 * d'agent, un proxy d'entreprise — où le certificat présenté est
 * légitimement signé par une autorité locale.
 *
 * C'est `--ignore-certificate-errors-spki-list` et non
 * `ignoreHTTPSErrors` : la première nomme les autorités qu'on accepte, la
 * seconde accepte tout. Un certificat expiré, ou signé par n'importe qui
 * d'autre, fait toujours échouer la navigation — ce qui est le but d'un
 * garde-fou.
 */
export const optionsChromium = () => {
  const options = {};
  if (process.env.PW_CHROMIUM) options.executablePath = process.env.PW_CHROMIUM;
  if (process.env.PLUM_SPKI) {
    options.args = [`--ignore-certificate-errors-spki-list=${process.env.PLUM_SPKI}`];
  }
  return options;
};

/**
 * Attend que le wasm ait pris la main sur la page.
 *
 * Sans cette attente, les gardes cliquent une pastille que le pré-rendu a
 * déjà peinte mais que personne n'écoute encore, et le clic ne fait rien. En
 * local le wasm arrive en quelques millisecondes et le défaut ne se voyait
 * pas ; contre l'origine déployée, six vérifications sur seize tombaient — un
 * défaut du garde, pas du site.
 *
 * Le signal : `DocumentHead` remplace, dans un effet, les `link[rel=alternate]`
 * de la coquille par les siens, qu'il marque `data-i18n-alternate`. Les effets
 * ne tournent pas au pré-rendu — la page livrée n'en porte aucun — donc cet
 * attribut n'apparaît qu'une fois l'hydratation faite. Si `DocumentHead`
 * cessait de le poser, l'attente expirerait : un échec bruyant, pas un vert
 * imméritée.
 */
export const attendreHydratation = (page) =>
  page.waitForSelector("link[rel=alternate][data-i18n-alternate]", {
    state: "attached",
    timeout: 15000,
  });
