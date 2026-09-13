import { useEffect } from "react";

import { useLanguage } from "../i18n";
import site from "../i18n/site.json";
import { languages } from "../i18n";
import type { PageKey } from "../i18n/routes";
import { pathFor } from "../i18n/routes";

function setMeta(selector: string, attribute: string, value: string) {
  const element = document.head.querySelector(selector);
  if (element) {
    element.setAttribute(attribute, value);
  }
}

/**
 * Keeps the parts of `<head>` that are language-dependent in step with the
 * page.
 *
 * The site is a single bundle with no server rendering, so `index.html` ships
 * one fixed `lang` and one fixed title. Leaving them would tell a screen
 * reader to pronounce Spanish with a French voice, and hand every crawler the
 * same French title for all three versions.
 */
export function DocumentHead({ page }: { page: PageKey }) {
  const language = useLanguage();

  useEffect(() => {
    const { title, description, pageTitles } = site.meta[language];
    const section = pageTitles[page];
    // The same rule the template generator applies, from the same table.
    const fullTitle = section === null ? title : `${section} — Plum`;

    document.documentElement.lang = language;
    document.title = fullTitle;
    setMeta('meta[name="description"]', "content", description);
    setMeta('meta[property="og:title"]', "content", fullTitle);
    setMeta('meta[property="og:description"]', "content", description);
  }, [language, page]);

  useEffect(() => {
    // Alternates tell a search engine these are the same page in another
    // language rather than three pages competing with each other.
    // Every `rel=alternate` goes, not just the ones added here: the
    // pre-rendered shell ships its own set for the URL that was fetched, and
    // after a client-side navigation those describe the page you came from.
    // Appending alongside them leaves two conflicting sets — which is exactly
    // what the browser test caught.
    const previous = document.head.querySelectorAll("link[rel=alternate]");
    previous.forEach((node) => node.remove());

    const added = [...languages, "x-default" as const].map((entry) => {
      const link = document.createElement("link");
      link.rel = "alternate";
      link.hreflang = entry === "x-default" ? "x-default" : entry;
      link.href = new URL(
        pathFor(entry === "x-default" ? "fr" : entry, page),
        window.location.origin,
      ).toString();
      link.setAttribute("data-i18n-alternate", "");
      document.head.append(link);
      return link;
    });

    return () => added.forEach((link) => link.remove());
  }, [page, language]);

  return null;
}
