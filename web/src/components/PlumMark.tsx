import { useId } from "react";

/**
 * La marque : une prune, dessinée plutôt qu'importée pour rester nette.
 *
 * Sa géométrie — le fruit, son sillon, sa queue — est la même que celle de
 * `public/favicon.svg` et de `Scripts/generate_appicon.py`, aux mêmes
 * coordonnées dans le même repère de 64 unités. Trois fichiers pour un seul
 * dessin, donc : si l'un change, les deux autres doivent suivre.
 */
export function PlumMark({ size = 64 }: { size?: number }) {
  // La marque paraît deux fois sur la même page — l'en-tête et le héros — et
  // un dégradé ou un masque se référence par identifiant. Deux identifiants
  // identiques dans un document ne sont pas seulement invalides : les deux
  // marques pointeraient sur la première définition, et le jour où celle-ci
  // disparaît du DOM, l'autre perd son sillon.
  const id = useId();
  const gradient = `plum-mark-${id}`;
  const groove = `plum-sillon-${id}`;

  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      role="img"
      aria-label="Plum"
    >
      <defs>
        <linearGradient id={gradient} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#E8608C" />
          <stop offset="100%" stopColor="#6B2D5C" />
        </linearGradient>
        {/*
         * Le sillon est creusé dans le fruit plutôt que tracé par-dessus :
         * c'est le dégradé du fond qui le remplit. Un trait posé dessus
         * aurait demandé une couleur de plus, fausse sur la moitié du
         * dégradé.
         */}
        <mask id={groove}>
          <rect width="64" height="64" fill="#fff" />
          <path
            d="M30 21 A 36.54 36.54 0 0 0 30 50"
            stroke="#000"
            strokeWidth="3.2"
            fill="none"
            strokeLinecap="round"
          />
        </mask>
      </defs>
      <circle cx="32" cy="32" r="32" fill={`url(#${gradient})`} />
      <g mask={`url(#${groove})`}>
        <ellipse cx="32" cy="35.5" rx="17" ry="18" fill="#FFF7F4" />
        {/* La queue part de l'intérieur du fruit, sinon elle flotte. */}
        <path
          d="M33 20 L 39.5 11"
          stroke="#FFF7F4"
          strokeWidth="3.4"
          fill="none"
          strokeLinecap="round"
        />
      </g>
    </svg>
  );
}
