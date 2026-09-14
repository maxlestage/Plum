/**
 * La marque : deux disques qui se chevauchent, et au milieu une troisième
 * forme que ni l'un ni l'autre n'a dessinée.
 *
 * Chaque moitié est un disque dont on retire un second disque décalé — d'où
 * le `fillRule="evenodd"` : ce qui appartient à l'un *ou* à l'autre, jamais
 * aux deux. C'est ce qui donne l'épaisseur variable d'un trait de plume,
 * épais sur les flancs et affiné en haut et en bas.
 *
 * La même géométrie, aux mêmes coordonnées dans le même repère de 64 unités,
 * est écrite dans `public/favicon.svg` et dans `Scripts/generate_appicon.py`.
 * Trois rendus d'un seul dessin : si l'un change, les deux autres doivent
 * suivre.
 *
 * Aucun dégradé et aucun identifiant : la marque est en aplat, donc rien à
 * référencer, donc rien à faire entrer en collision quand elle paraît deux
 * fois sur la même page.
 */
export function PlumMark({ size = 64 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      role="img"
      aria-label="Plum"
    >
      <circle cx="32" cy="32" r="32" fill="#40183A" />
      <path
        d="M23.5 14.5 a17.5 17.5 0 1 0 0 35 a17.5 17.5 0 1 0 0 -35 Z M30 16.8 a15.2 15.2 0 1 1 0 30.4 a15.2 15.2 0 1 1 0 -30.4 Z"
        fill="#FFF7F4"
        fillRule="evenodd"
      />
      <path
        d="M40.5 14.5 a17.5 17.5 0 1 1 0 35 a17.5 17.5 0 1 1 0 -35 Z M34 16.8 a15.2 15.2 0 1 0 0 30.4 a15.2 15.2 0 1 0 0 -30.4 Z"
        fill="#FFF7F4"
        fillRule="evenodd"
      />
    </svg>
  );
}
