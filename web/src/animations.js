/**
 * Le mouvement de la page d'accueil.
 *
 * Dans l'esprit de zamocorp.com — une ouverture épinglée que le défilement
 * déroule, une marque qui se forme dans la poussière, un balayage de lumière,
 * des boutons aimantés — mais sans ce qui pèse : là où ce site télécharge trois
 * cents images pré-rendues, la poussière est calculée ici, depuis la géométrie
 * exacte de la marque, en quelques kilo-octets.
 *
 * Trois règles, parce que ce script s'ajoute à une page que Yew hydrate :
 *
 * - il ne crée, ne retire et ne déplace **aucun nœud** dans `#root`. Le
 *   moindre nœud de trop et l'hydratation trouve un DOM qu'elle n'a pas
 *   produit. Il dessine dans une toile que le composant rend lui-même, et il
 *   écrit ses variables sur `<html>`, que Yew ne gère pas ;
 * - il ne touche à un élément de l'application qu'**après** l'hydratation, et
 *   la reconnaît à la marque que seul l'arbre hydraté pose ;
 * - il se tait si la personne a demandé de réduire les animations, et la page
 *   reste complète sans lui : sans `--ouverture`, le CSS montre l'état final.
 */

const MOUVEMENT_REDUIT = matchMedia("(prefers-reduced-motion: reduce)");
const racine = document.documentElement;

/** Posée par `DocumentHead` sur les alternates qu'il substitue, dans un effet :
 *  elle n'existe qu'une fois l'arbre hydraté. */
const hydratee = () => document.querySelector("link[data-i18n-alternate]") !== null;

const borne = (valeur, bas = 0, haut = 1) => Math.min(haut, Math.max(bas, valeur));
const sortieCubique = (t) => 1 - Math.pow(1 - t, 3);

/* ------------------------------------------------------------------------ *
 *  La poussière                                                            *
 * ------------------------------------------------------------------------ */

/**
 * Chaque moitié de la marque : un grand disque dont on retire un petit disque
 * décalé — ce qui appartient à l'un *ou* à l'autre. Mêmes valeurs, même repère
 * de 64 unités que `Scripts/generate_appicon.py` ; `Scripts/check_mark.py`
 * vérifie qu'elles ne s'écartent pas.
 */
const GAUCHE = [[23.5, 32.0, 17.5], [30.0, 32.0, 15.2]];
const DROITE = [[40.5, 32.0, 17.5], [34.0, 32.0, 15.2]];

const dansDisque = (x, y, [cx, cy, r]) => (x - cx) ** 2 + (y - cy) ** 2 <= r * r;
const dansMoitie = (x, y, [grand, petit]) => dansDisque(x, y, grand) !== dansDisque(x, y, petit);
const dansLaMarque = (x, y) => dansMoitie(x, y, GAUCHE) || dansMoitie(x, y, DROITE);

// L'encombrement de la marque dans son repère, déduit de la géométrie.
const BOITE = { gauche: 6, droite: 58, haut: 14.5, bas: 49.5 };

function echantillonner(nombre) {
  const points = [];
  while (points.length < nombre) {
    const x = BOITE.gauche + Math.random() * (BOITE.droite - BOITE.gauche);
    const y = BOITE.haut + Math.random() * (BOITE.bas - BOITE.haut);
    if (!dansLaMarque(x, y)) continue;
    points.push({
      cx: x,
      cy: y,
      // D'où vient chaque grain, en fraction de la scène — redéfini à chaque
      // changement de taille.
      depart: [Math.random() * 1.4 - 0.2, Math.random() * 1.4 - 0.2],
      // La marque se forme de gauche à droite, avec du désordre dedans.
      retard: 0.12 * Math.random() + 0.1 * ((x - BOITE.gauche) / (BOITE.droite - BOITE.gauche)),
      phase: Math.random() * Math.PI * 2,
      taille: 0.9 + Math.random() * 0.8,
      // Un grain sur sept dans la couleur chaude : sans eux, une marque
      // d'une seule encre se lit comme un gabarit.
      chaud: Math.random() < 0.14,
    });
  }
  return points;
}

/** Où en est le défilement de l'ouverture, de 0 à 1. */
function progression(section) {
  const cadre = section.getBoundingClientRect();
  const course = cadre.height - innerHeight;
  return course > 0 ? borne(-cadre.top / course) : 1;
}

function demarrerLaPoussiere() {
  const points = echantillonner(innerWidth < 600 ? 1400 : 2400);
  let toile = null;
  let section = null;
  let visible = true;
  let arrete = false;
  let image = 0;
  let observateur = null;
  let derniere = -1;

  /** La toile est rendue par le composant ; une navigation la remplace. */
  function retrouver() {
    if (toile?.isConnected) return true;
    toile = document.querySelector(".ouverture__poussiere");
    section = toile?.closest(".ouverture") ?? null;
    observateur?.disconnect();
    if (!toile || !section) return false;
    observateur = new IntersectionObserver(([entree]) => {
      visible = entree.isIntersecting;
      if (visible) programmer();
    });
    observateur.observe(section);
    return true;
  }

  function programmer() {
    if (!arrete && !image) image = requestAnimationFrame(dessiner);
  }

  function dessiner(instant) {
    image = 0;
    if (arrete || !retrouver()) return;

    const p = progression(section);
    // Le balayage de lumière traverse une fois la marque presque formée.
    const lueur = borne((p - 0.4) / 0.26);
    racine.style.setProperty("--ouverture", p.toFixed(4));
    racine.style.setProperty("--lueur", lueur.toFixed(4));

    // Tant que la poussière flotte, chaque image compte ; une fois la marque
    // formée et la lueur passée, on ne redessine qu'au défilement.
    const vivante = p < 0.5 || (lueur > 0 && lueur < 1);
    if (!visible) return;
    if (!vivante && Math.abs(p - derniere) < 0.0005) return;
    derniere = p;

    const ratio = Math.min(devicePixelRatio || 1, 2);
    const largeur = toile.clientWidth;
    const hauteur = toile.clientHeight;
    // Revérifié à chaque image : si l'hydratation a rendu à la toile ses
    // attributs d'origine, elle reprend sa taille ici plutôt que de rester
    // à 300 × 150, floue.
    if (toile.width !== Math.round(largeur * ratio) || toile.height !== Math.round(hauteur * ratio)) {
      toile.width = Math.round(largeur * ratio);
      toile.height = Math.round(hauteur * ratio);
    }
    const ctx = toile.getContext("2d");
    if (!ctx || !largeur || !hauteur) return programmer();
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, largeur, hauteur);

    // La marque occupe le haut de la scène ; le texte, le bas.
    const echelle = Math.min(largeur * 0.78, 360, hauteur * 0.5) / (BOITE.droite - BOITE.gauche);
    const ox = largeur / 2 - 32 * echelle;
    const oy = hauteur * 0.36 - 32 * echelle;
    // La même course que le trait de `.ouverture__lueur`, de bord à bord.
    const xLueur = lueur * (largeur - 2);

    const style = getComputedStyle(racine);
    const encre = style.getPropertyValue("--plum-contrast").trim() || "#6b2d5c";
    const chaud = style.getPropertyValue("--blush").trim() || "#e8608c";
    const t = instant / 1000;

    const lots = { [encre]: [], [chaud]: [] };
    for (const grain of points) {
      const a = sortieCubique(borne((p - grain.retard) / 0.3));
      const flotte = (1 - a) * 22;
      const sx = grain.depart[0] * largeur + Math.sin(t * 0.6 + grain.phase) * flotte;
      const sy = grain.depart[1] * hauteur + Math.cos(t * 0.5 + grain.phase) * flotte;
      const x = sx + (ox + grain.cx * echelle - sx) * a;
      const y = sy + (oy + grain.cy * echelle - sy) * a;
      // Les grains que la lueur touche s'allument et grossissent.
      const touche = lueur > 0 && lueur < 1 && Math.abs(x - xLueur) < 26;
      const taille = grain.taille * (touche ? 2.2 : 1);
      (touche || grain.chaud ? lots[chaud] : lots[encre]).push(x, y, taille);
    }
    for (const [couleur, valeurs] of Object.entries(lots)) {
      ctx.fillStyle = couleur;
      for (let i = 0; i < valeurs.length; i += 3) {
        ctx.fillRect(valeurs[i], valeurs[i + 1], valeurs[i + 2], valeurs[i + 2]);
      }
    }

    if (vivante) programmer();
  }

  addEventListener("scroll", programmer, { passive: true });
  addEventListener("resize", () => {
    derniere = -1;
    programmer();
  });
  // Une navigation côté client peut remplacer la toile : on la recherche
  // à la prochaine image, et `retrouver` s'en charge.
  addEventListener("popstate", programmer);
  document.addEventListener("click", () => requestAnimationFrame(programmer));
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) programmer();
  });
  programmer();

  return {
    arreter() {
      arrete = true;
      if (image) cancelAnimationFrame(image);
      observateur?.disconnect();
      if (toile) toile.getContext("2d")?.clearRect(0, 0, toile.width, toile.height);
    },
  };
}

/* ------------------------------------------------------------------------ *
 *  Les aimants                                                             *
 * ------------------------------------------------------------------------ */

/**
 * Les boutons de l'ouverture suivent le pointeur, puis reviennent en rebondissant.
 *
 * Seulement avec une souris : au doigt il n'y a pas de survol, et un bouton
 * qui bougerait sous le pouce au moment de le toucher serait un piège.
 * Délégué depuis le document plutôt qu'attaché à chaque bouton, pour survivre
 * aux re-rendus de Yew sans rien avoir à rebrancher.
 */
function demarrerLesAimants() {
  if (!matchMedia("(pointer: fine)").matches) return;
  let actif = null;

  const relacher = (bouton) => {
    bouton.style.removeProperty("--mx");
    bouton.style.removeProperty("--my");
  };

  document.addEventListener(
    "pointermove",
    (evenement) => {
      const cible = evenement.target instanceof Element ? evenement.target.closest(".aimant") : null;
      if (actif && actif !== cible) relacher(actif);
      actif = cible;
      // Pas avant l'hydratation : un attribut `style` posé sur un nœud que
      // Yew n'a pas encore adopté serait un écart qu'il n'a pas produit.
      if (!cible || MOUVEMENT_REDUIT.matches || !hydratee()) return;
      const cadre = cible.getBoundingClientRect();
      const dx = evenement.clientX - (cadre.left + cadre.width / 2);
      const dy = evenement.clientY - (cadre.top + cadre.height / 2);
      cible.style.setProperty("--mx", `${(dx * 0.3).toFixed(1)}px`);
      cible.style.setProperty("--my", `${(dy * 0.35).toFixed(1)}px`);
    },
    { passive: true },
  );
  document.addEventListener("pointerleave", () => actif && relacher(actif));
}

/* ------------------------------------------------------------------------ *
 *  Le démarrage                                                            *
 * ------------------------------------------------------------------------ */

// En fin de fichier, et pas plus haut : il lit `GAUCHE`, `DROITE` et `BOITE`,
// et une `const` n'existe qu'à partir de sa ligne. Placé en tête, il levait
// « Cannot access 'BOITE' before initialization », la poussière ne partait
// jamais, et la lueur restait plantée à droite de l'écran — c'est le garde de
// mise en page qui l'a vu.
if (!MOUVEMENT_REDUIT.matches) {
  racine.classList.add("anime");
  const poussiere = demarrerLaPoussiere();
  demarrerLesAimants();

  // Quelqu'un qui active « réduire les animations » en cours de route
  // retrouve la page immobile, sans recharger.
  MOUVEMENT_REDUIT.addEventListener("change", (evenement) => {
    if (!evenement.matches) return;
    racine.classList.remove("anime");
    racine.style.removeProperty("--ouverture");
    racine.style.removeProperty("--lueur");
    poussiere.arreter();
  });
}
