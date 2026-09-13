import site from "./site.json";
import type { Language } from "./types";

/**
 * The page keys, and the slug each language uses for them.
 *
 * The table lives in `site.json` because the build-time template generator
 * needs the same one, and it cannot import TypeScript.
 *
 * Translated slugs rather than `/es/privacy`: a Spanish reader should not have
 * to read English to know where a link goes. The key is what the code routes
 * on, so the slugs can change without touching a component.
 */
export const pages = ["home", "terms", "privacy", "help"] as const;

export type PageKey = (typeof pages)[number];

export const slugs = site.slugs as Record<Language, Record<PageKey, string>>;

/**
 * Always with a trailing slash.
 *
 * The pages are served as directory indexes, and `ServeDir` answers a
 * directory URL without a trailing slash with a 307 to the version that has
 * one. Declaring the slashless form in `hreflang` would point every
 * translation at a URL that redirects before it serves, and would leave the
 * address bar disagreeing with the canonical after a reload.
 */
export function pathFor(language: Language, page: PageKey): string {
  const slug = slugs[language][page];
  return slug === "" ? `/${language}/` : `/${language}/${slug}/`;
}

/** Which page a slug refers to, in any language, or `undefined`. */
export function pageForSlug(
  language: Language,
  slug: string | undefined,
): PageKey | undefined {
  const table = slugs[language];
  if (slug === undefined || slug === "") {
    return "home";
  }
  return pages.find((page) => table[page] === slug);
}
