import { Link, useLocation } from "react-router-dom";
import type { ReactNode } from "react";

import { PlumMark } from "./PlumMark";

export function Layout({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();

  return (
    <>
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
            to="/"
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
          {pathname !== "/" && (
            <Link to="/" className="muted" style={{ marginInlineStart: "auto" }}>
              Accueil
            </Link>
          )}
        </div>
      </header>

      <main>{children}</main>

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
            <Link to="/conditions">Conditions d'utilisation</Link>
            <Link to="/confidentialite">Confidentialité</Link>
            <Link to="/aide">Aide et contact</Link>
          </nav>
          <p className="muted" style={{ margin: 0, fontSize: "0.9rem" }}>
            Plum — des rencontres pas sérieuses. Application réservée aux
            personnes majeures.
          </p>
        </div>
      </footer>
    </>
  );
}
