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
const { slugs, meta } = site;

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

let written = 0;

for (const language of languages) {
  for (const page of pages) {
    const { title, description, pageTitles } = meta[language];
    const section = pageTitles[page];
    const fullTitle = section === null ? title : `${section} — Plum`;

    const alternates = [...languages, "x-default"]
      .map((entry) => {
        const target = entry === "x-default" ? "fr" : entry;
        return `<link rel="alternate" hreflang="${entry}" href="${pathFor(target, page)}" />`;
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
        `<meta property="og:description" content="${escapeHtml(description)}" />\n    <meta property="og:locale" content="${language}" />\n    <link rel="canonical" href="${pathFor(language, page)}" />\n    ${alternates}`,
      );

    const directory = path.join(dist, pathFor(language, page));
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, "index.html"), html);
    written++;
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
  'rel="canonical" href="/es/privacidad/"',
]) {
  if (!sample.includes(expected)) {
    console.error(
      `prerender : « ${expected} » absent du témoin, le gabarit a dû changer`,
    );
    process.exit(1);
  }
}

console.log(`prerender : ${written} pages écrites`);
