import { useEffect } from "react";

import { useCopy, useLanguage } from "../i18n";
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
  const copy = useCopy();
  const language = useLanguage();

  useEffect(() => {
    document.documentElement.lang = copy.htmlLang;
    document.title = copy.documentTitle;
    setMeta('meta[name="description"]', "content", copy.metaDescription);
    setMeta('meta[property="og:title"]', "content", copy.documentTitle);
    setMeta('meta[property="og:description"]', "content", copy.metaDescription);
  }, [copy]);

  useEffect(() => {
    // Alternates tell a search engine these are the same page in another
    // language rather than three pages competing with each other.
    const previous = document.head.querySelectorAll(
      "link[data-i18n-alternate]",
    );
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
