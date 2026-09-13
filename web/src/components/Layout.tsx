import { Link } from "react-router-dom";
import type { ReactNode } from "react";

import { useCopy, useLanguage } from "../i18n";
import type { PageKey } from "../i18n/routes";
import { pathFor } from "../i18n/routes";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { PlumMark } from "./PlumMark";

export function Layout({
  page,
  children,
}: {
  page: PageKey;
  children: ReactNode;
}) {
  const copy = useCopy();
  const language = useLanguage();

  return (
    <>
      <a className="skip-link" href="#main">
        {copy.nav.skipToContent}
      </a>

      <header
        style={{
          borderBottom: "1px solid var(--hairline)",
          background: "var(--surface)",
        }}
      >
        <div
          className="shell"
          style={{
            display: "flex",
            alignItems: "center",
            gap: 12,
            minHeight: 64,
          }}
        >
          <Link
            to={pathFor(language, "home")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 10,
              textDecoration: "none",
              color: "inherit",
              fontWeight: 700,
              fontSize: "1.1rem",
            }}
          >
            <PlumMark size={30} />
            Plum
          </Link>
          <LanguageSwitcher page={page} />
        </div>
      </header>

      <main id="main">{children}</main>

      <footer
        style={{
          borderTop: "1px solid var(--hairline)",
          padding: "40px 0",
          marginTop: 40,
        }}
      >
        <div className="shell">
          <nav
            style={{
              display: "flex",
              flexWrap: "wrap",
              gap: "12px 24px",
              marginBottom: 16,
            }}
          >
            {page !== "home" && (
              <Link to={pathFor(language, "home")}>{copy.nav.home}</Link>
            )}
            <Link to={pathFor(language, "terms")}>{copy.nav.terms}</Link>
            <Link to={pathFor(language, "privacy")}>{copy.nav.privacy}</Link>
            <Link to={pathFor(language, "help")}>{copy.nav.help}</Link>
          </nav>
          <p className="muted" style={{ margin: 0, fontSize: "0.9rem" }}>
            {copy.footerTagline}
          </p>
        </div>
      </footer>
    </>
  );
}
