import { StrictMode } from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";

import { App } from "./App";
import "./theme.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("élément #root introuvable");
}

const tree = (
  <StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </StrictMode>
);

// Les douze pages traduites arrivent déjà rendues : on les hydrate, ce qui
// évite de reconstruire un DOM identique et laisse le texte lisible avant
// même que React ne démarre.
//
// La racine, elle, n'est qu'une redirection vers la langue détectée : rien à
// rendre au moment de la construction, donc rien à hydrater. Hydrater un
// conteneur vide fonctionnerait — React refait la page — mais en se plaignant
// à chaque chargement, et un avertissement permanent est un avertissement
// qu'on cesse de lire.
if (container.firstChild) {
  hydrateRoot(container, tree);
} else {
  createRoot(container).render(tree);
}
