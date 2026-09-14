import { Link } from "react-router-dom";

import { languageNames, languages, useCopy, useLanguage } from "../i18n";
import type { PageKey } from "../i18n/routes";
import { pathFor } from "../i18n/routes";

/**
 * Trois codes courts, la langue courante en pastille pleine.
 *
 * Des liens, pas un `<select>` : un menu déroulant demande du JavaScript pour
 * naviguer et ne donne rien à suivre à un robot. Trois liens s'explorent, se
 * clique-molettent, et fonctionnent pareil sur un téléphone. Ils gardent la
 * page qu'on lisait plutôt que de renvoyer à l'accueil, ce qui est justement
 * ce qui rend un sélecteur agaçant.
 *
 * Le code est ce qu'on voit ; le nom complet est ce qu'on entend. « FR » lu à
 * voix haute par un lecteur d'écran ne veut rien dire — `aria-label` porte
 * donc l'endonyme, « Français », et la langue de ce mot est déclarée avec lui,
 * sans quoi une voix française prononcerait « Español » à la française.
 */
export function LanguageSwitcher({ page }: { page: PageKey }) {
  const current = useLanguage();
  const copy = useCopy();

  return (
    <nav className="langues" aria-label={copy.nav.languageLabel}>
      {languages.map((language) => {
        const code = language.toUpperCase();
        const name = languageNames[language];

        return language === current ? (
          <span
            key={language}
            className="langue langue--active"
            aria-current="true"
            aria-label={name}
            lang={language}
          >
            {code}
          </span>
        ) : (
          <Link
            key={language}
            to={pathFor(language, page)}
            className="langue"
            // `hreflang` dit au navigateur et au robot ce qu'il y a au bout
            // avant qu'ils y aillent ; `lang` dit comment prononcer l'étiquette.
            hrefLang={language}
            lang={language}
            aria-label={name}
          >
            {code}
          </Link>
        );
      })}
    </nav>
  );
}
