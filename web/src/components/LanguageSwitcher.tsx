import { Link } from "react-router-dom";

import { languageNames, languages, useCopy, useLanguage } from "../i18n";
import type { PageKey } from "../i18n/routes";
import { pathFor } from "../i18n/routes";

/**
 * Plain links, not a `<select>`.
 *
 * A select needs JavaScript to navigate and gives a crawler nothing to
 * follow; three links are crawlable, middle-clickable, and work the same on a
 * phone. They keep you on the page you were reading rather than dropping you
 * back on the home page, which is the thing that makes a switcher annoying.
 */
export function LanguageSwitcher({ page }: { page: PageKey }) {
  const current = useLanguage();
  const copy = useCopy();

  return (
    <nav
      aria-label={copy.nav.languageLabel}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 4,
        marginInlineStart: "auto",
      }}
    >
      {languages.map((language) =>
        language === current ? (
          <span
            key={language}
            aria-current="true"
            style={{
              padding: "4px 8px",
              borderRadius: 999,
              fontSize: "0.85rem",
              fontWeight: 700,
              background: "color-mix(in srgb, var(--plum) 12%, transparent)",
              color: "var(--plum)",
            }}
          >
            {languageNames[language]}
          </span>
        ) : (
          <Link
            key={language}
            to={pathFor(language, page)}
            // `hreflang` tells a browser and a crawler what is on the other
            // end before they follow it.
            hrefLang={language}
            className="muted"
            style={{
              padding: "4px 8px",
              borderRadius: 999,
              fontSize: "0.85rem",
              textDecoration: "none",
            }}
          >
            {languageNames[language]}
          </Link>
        ),
      )}
    </nav>
  );
}
