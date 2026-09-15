import { useCopy } from "../i18n";
import type { ThemeChoice } from "../theme";
import { setTheme, themeChoices, useTheme } from "../theme";

/**
 * Automatique, clair, sombre — dans cet ordre, et « automatique » d'abord
 * parce que c'est l'état par défaut et celui auquel on revient.
 *
 * Des boutons, pas des liens : le thème est une préférence d'appareil, pas une
 * adresse. Et des dessins, pas des caractères : ☀ et ☾ sont rendus en émoji
 * couleur par certains systèmes et en glyphe noir par d'autres, ce qui donne
 * une rangée dont un tiers ne ressemble pas au reste. Un SVG en
 * `currentColor` se comporte pareil partout et vire au crème sur la pastille
 * pleine.
 *
 * `aria-pressed` plutôt que `aria-current` : trois boutons dont un est enfoncé,
 * c'est ce que c'est. `aria-current` désigne l'endroit où l'on est dans une
 * navigation, et ceci n'est pas une navigation.
 */
export function ThemeSwitcher() {
  const copy = useCopy();
  const current = useTheme();

  return (
    <div className="bascules" role="group" aria-label={copy.nav.themeLabel}>
      {themeChoices.map((choice) => (
        <button
          key={choice}
          type="button"
          className={
            choice === current ? "pastille pastille--active" : "pastille"
          }
          aria-pressed={choice === current}
          // Le dessin ne se lit pas à voix haute : l'étiquette porte le mot.
          aria-label={copy.nav.themes[choice]}
          title={copy.nav.themes[choice]}
          onClick={() => setTheme(choice)}
        >
          <Glyphe choice={choice} />
        </button>
      ))}
    </div>
  );
}

function Glyphe({ choice }: { choice: ThemeChoice }) {
  const commun = {
    viewBox: "0 0 16 16",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.4,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    // Le dessin est décoratif : son sens est déjà dans l'`aria-label` du
    // bouton, et le répéter ferait entendre l'étiquette deux fois.
    "aria-hidden": true,
    focusable: false,
  };

  if (choice === "light") {
    return (
      <svg {...commun}>
        <circle cx="8" cy="8" r="3.1" />
        {[0, 45, 90, 135, 180, 225, 270, 315].map((angle) => (
          <line
            key={angle}
            x1="8"
            y1="1.6"
            x2="8"
            y2="3.2"
            transform={`rotate(${angle} 8 8)`}
          />
        ))}
      </svg>
    );
  }

  if (choice === "dark") {
    // Un croissant d'un seul trait : deux arcs qui se rejoignent. Découper un
    // disque par un second disque donnerait une forme qui disparaît dès que le
    // fond passe au prune plein.
    return (
      <svg {...commun}>
        <path d="M13.2 10.1A5.8 5.8 0 0 1 5.9 2.8a5.8 5.8 0 1 0 7.3 7.3Z" />
      </svg>
    );
  }

  // Automatique : un disque à moitié plein, la moitié claire et la moitié
  // sombre du même objet.
  return (
    <svg {...commun}>
      <circle cx="8" cy="8" r="5.8" />
      <path d="M8 2.2a5.8 5.8 0 0 0 0 11.6Z" fill="currentColor" stroke="none" />
    </svg>
  );
}
