/**
 * Writes a static HTML shell for every language and page.
 *
 * The site is one bundle with no server rendering, so `index.html` carries a
 * single fixed `lang`, title and description. The app fixes them once React
 * runs — which is fine for a person and useless for the crawlers that build
 * link previews: sharing `/es` would show the French title in WhatsApp,
 * iMessage or Slack, because none of them executes JavaScript.
 *
 * So each page gets its own shell with the right metadata baked in. The
 * bundle is identical; only the head differs. `ServeDir` serves
 * `/es/privacidad/index.html` for `/es/privacidad` on its own, and the SPA
 * takes over from there.
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const dist = path.join(here, "..", "dist");

const languages = ["fr", "en", "es"];
const pages = ["home", "terms", "privacy", "help"];

/**
 * Slugs and metadata come from `src/i18n/site.json`, the same file the
 * application reads. They used to be copied here, and a copy of a title is a
 * title that goes stale the first time someone edits only one of them.
 */
const site = JSON.parse(
  fs.readFileSync(path.join(here, "..", "src", "i18n", "site.json"), "utf8"),
);
const { slugs, meta, origin } = site;

/**
 * Une adresse absolue.
 *
 * Google ignore un `hreflang` relatif, et recommande un `canonical` absolu.
 * Les coquilles sont précisément ce que les robots lisent — l'application, à
 * l'exécution, les réécrit déjà en absolu à partir de `window.location.origin`,
 * donc l'erreur ne se voyait que là où elle comptait.
 */
function absolute(pathname) {
  return `${origin}${pathname}`;
}

/**
 * Les balises de l'aperçu de lien.
 *
 * Sans elles, un lien envoyé sur WhatsApp, iMessage ou Slack s'affiche en
 * texte nu. `og:image` doit être absolue — un aperçu est construit par un
 * serveur qui n'a aucune page de référence pour résoudre un chemin — d'où
 * leur place ici plutôt que dans le gabarit, où l'adresse du déploiement
 * devrait être recopiée.
 *
 * `summary_large_image` demande la grande vignette plutôt que la miniature
 * carrée, qui rognerait la marque.
 */
function preview(language) {
  return [
    `<meta property="og:image" content="${absolute("/partage.png")}" />`,
    `<meta property="og:image:width" content="1200" />`,
    `<meta property="og:image:height" content="630" />`,
    `<meta property="og:image:alt" content="${escapeHtml(meta[language].imageAlt)}" />`,
    `<meta name="twitter:card" content="summary_large_image" />`,
  ].join("\n    ");
}

/** `&` first, or it would double-escape the entities added after it. */
function escapeHtml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

/** Trailing slash, for the same reason as its twin in `src/i18n/routes.ts`. */
function pathFor(language, page) {
  const slug = slugs[language][page];
  return slug === "" ? `/${language}/` : `/${language}/${slug}/`;
}

const template = fs.readFileSync(path.join(dist, "index.html"), "utf8");

/*
 * Le gabarit est aussi une des sorties : la racine reçoit ses annotations
 * comme les douze autres pages. Relancé sans `vite build`, ce script lirait
 * donc sa propre production et injecterait tout une seconde fois — quatre
 * balises en double, qu'aucun aperçu ne sait départager.
 *
 * Trouvé en testant autre chose, ce qui est la seule raison pour laquelle ce
 * garde-fou existe : `npm run build` reconstruit toujours `dist` avant, donc
 * le défaut ne se serait vu qu'un jour où quelqu'un lance l'étape seule.
 */
if (template.includes('rel="canonical"')) {
  console.error(
    "prerender : dist/index.html porte déjà ses annotations — relancez `npm run build`,\n" +
      "            ce script doit partir d'une construction fraîche.",
  );
  process.exit(1);
}

let written = 0;

for (const language of languages) {
  for (const page of pages) {
    const { title, description, pageTitles } = meta[language];
    const section = pageTitles[page];
    const fullTitle = section === null ? title : `${section} — Plum`;

    const alternates = [...languages, "x-default"]
      .map((entry) => {
        const target = entry === "x-default" ? "fr" : entry;
        return `<link rel="alternate" hreflang="${entry}" href="${absolute(pathFor(target, page))}" />`;
      })
      .join("\n    ");

    const html = template
      .replace('<html lang="fr">', `<html lang="${language}">`)
      .replace(
        /<title>[^<]*<\/title>/,
        `<title>${escapeHtml(fullTitle)}</title>`,
      )
      // The description and og:description meta tags are written across
      // several lines by the formatter, hence the non-greedy match.
      .replace(
        /<meta\s+name="description"[\s\S]*?\/>/,
        `<meta name="description" content="${escapeHtml(description)}" />`,
      )
      .replace(
        /<meta\s+property="og:title"[\s\S]*?\/>/,
        `<meta property="og:title" content="${escapeHtml(fullTitle)}" />`,
      )
      .replace(
        /<meta\s+property="og:description"[\s\S]*?\/>/,
        `<meta property="og:description" content="${escapeHtml(description)}" />\n    <meta property="og:locale" content="${language}" />\n    ${preview(language)}\n    <link rel="canonical" href="${absolute(pathFor(language, page))}" />\n    ${alternates}`,
      );

    const directory = path.join(dist, pathFor(language, page));
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, "index.html"), html);
    written++;
  }
}

/**
 * La racine, qui est l'adresse qu'on partage.
 *
 * `/` n'est pas une page : l'application y choisit une langue et renvoie vers
 * `/fr/`. Mais c'est une redirection côté client, donc un robot qui n'exécute
 * pas de JavaScript s'arrête là — et il y trouvait le gabarit brut, sans
 * `canonical` ni `hreflang`, précisément sur l'adresse la plus liée du site.
 *
 * Elle porte donc maintenant les annotations de la version française, vers
 * laquelle elle mène.
 */
{
  const { title, description } = meta.fr;
  const alternates = [...languages, "x-default"]
    .map((entry) => {
      const target = entry === "x-default" ? "fr" : entry;
      return `<link rel="alternate" hreflang="${entry}" href="${absolute(pathFor(target, "home"))}" />`;
    })
    .join("\n    ");

  const root = template
    .replace(
      /<meta\s+property="og:description"[\s\S]*?\/>/,
      `<meta property="og:description" content="${escapeHtml(description)}" />\n    <meta property="og:locale" content="fr" />\n    ${preview("fr")}\n    <link rel="canonical" href="${absolute(pathFor("fr", "home"))}" />\n    ${alternates}`,
    );

  if (!root.includes('rel="canonical"')) {
    console.error("prerender : la racine n'a pas reçu son canonical");
    process.exit(1);
  }
  fs.writeFileSync(path.join(dist, "index.html"), root);
  written++;
  // `title` et `description` du gabarit sont déjà ceux de l'accueil français,
  // donc rien à y réécrire — vérifié plutôt que supposé.
  if (!root.includes(`<title>${escapeHtml(title)}</title>`)) {
    console.error("prerender : le gabarit ne porte plus le titre français");
    process.exit(1);
  }
}

// A shell that made none of its substitutions would ship silently and look
// fine to a person while staying wrong for every crawler.
const sample = fs.readFileSync(
  path.join(dist, "es", "privacidad", "index.html"),
  "utf8",
);
for (const expected of [
  '<html lang="es">',
  "Privacidad — Plum",
  'hreflang="x-default"',
  `rel="canonical" href="${origin}/es/privacidad/"`,
  `hreflang="fr" href="${origin}/fr/confidentialite/"`,
  `og:image" content="${origin}/partage.png"`,
  'name="twitter:card" content="summary_large_image"',
]) {
  if (!sample.includes(expected)) {
    console.error(
      `prerender : « ${expected} » absent du témoin, le gabarit a dû changer`,
    );
    process.exit(1);
  }
}

console.log(`prerender : ${written} pages écrites`);
