import { StrictMode } from "react";
import { renderToString } from "react-dom/server";
import { StaticRouter } from "react-router-dom/server";

import { App } from "./App";
import "./theme.css";

/**
 * Rend une page en HTML, au moment de la construction.
 *
 * Les coquilles portaient jusqu'ici les bonnes métadonnées et un corps vide :
 * un robot qui n'exécute pas de JavaScript voyait le bon titre et rien
 * dessous. Les aperçus de lien s'en accommodaient — ils lisent les balises —
 * mais pas les moteurs qui ne rendent pas le JavaScript, ni personne dont le
 * navigateur peine à le charger.
 *
 * `StaticRouter` plutôt que `BrowserRouter` : il n'y a pas d'historique ici,
 * seulement une adresse. Le reste de l'arbre est identique à celui du client,
 * sans quoi l'hydratation trouverait un DOM qu'elle n'a pas produit.
 */
export function render(pathname: string): string {
  return renderToString(
    <StrictMode>
      <StaticRouter location={pathname}>
        <App />
      </StaticRouter>
    </StrictMode>,
  );
}
