import { useEffect, useRef } from "react";

import { useCopy } from "../i18n";
import type { PageKey } from "../i18n/routes";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { ThemeSwitcher } from "./ThemeSwitcher";

/**
 * Les six pastilles rangées derrière un bouton, pour que l'en-tête tienne sur
 * une ligne avec le nom du site.
 *
 * `<details>` plutôt qu'un bouton et un état React, et ce n'est pas une
 * coquetterie : le site est pré-rendu pour rester lisible sans JavaScript, et
 * `<details>` s'ouvre tout seul. Sans lui, quelqu'un dont le script ne charge
 * pas se retrouverait sans aucun moyen de changer de langue. Le navigateur
 * l'annonce aussi correctement aux lecteurs d'écran, sans qu'on ait à tenir
 * `aria-expanded` à la main — un attribut qu'on oublie de mettre à jour est
 * pire que pas d'attribut du tout.
 *
 * Le JavaScript ne fait qu'ajouter le confort : refermer à l'Échap et au clic
 * au-dehors. Sans lui, on referme en retouchant le bouton, ce qui marche.
 */
export function HeaderMenu({ page }: { page: PageKey }) {
  const copy = useCopy();
  const boite = useRef<HTMLDetailsElement>(null);

  useEffect(() => {
    const fermer = () => {
      if (boite.current) boite.current.open = false;
    };

    const auClic = (evenement: MouseEvent) => {
      const cible = evenement.target;
      if (
        boite.current?.open &&
        cible instanceof Node &&
        !boite.current.contains(cible)
      ) {
        fermer();
      }
    };

    const auClavier = (evenement: KeyboardEvent) => {
      if (evenement.key === "Escape") fermer();
    };

    // `pointerdown` et non `click` : un menu qui se referme au relâchement
    // laisse le doigt sur ce qu'il y avait dessous.
    document.addEventListener("pointerdown", auClic);
    document.addEventListener("keydown", auClavier);
    return () => {
      document.removeEventListener("pointerdown", auClic);
      document.removeEventListener("keydown", auClavier);
    };
  }, []);

  return (
    <details className="menu" ref={boite}>
      <summary className="menu__bouton" aria-label={copy.nav.menuLabel}>
        <svg
          viewBox="0 0 20 20"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.7"
          strokeLinecap="round"
          aria-hidden="true"
          focusable="false"
        >
          <line x1="3" y1="6" x2="17" y2="6" />
          <line x1="3" y1="10" x2="17" y2="10" />
          <line x1="3" y1="14" x2="17" y2="14" />
        </svg>
      </summary>

      {/*
        Les deux groupes portent enfin un intitulé visible. Six pastilles en
        rang dans l'en-tête ne disaient pas lesquelles étaient des langues et
        lesquelles des thèmes ; ici, la place existe pour le dire.
      */}
      <div className="menu__panneau">
        <p className="menu__titre">{copy.nav.languageLabel}</p>
        <LanguageSwitcher page={page} />
        <p className="menu__titre">{copy.nav.themeLabel}</p>
        <ThemeSwitcher />
      </div>
    </details>
  );
}
