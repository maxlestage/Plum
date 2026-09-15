import { useSyncExternalStore } from "react";

/**
 * Le thème : automatique, clair, sombre.
 *
 * « Automatique » est la valeur par défaut et n'écrit rien — c'est la feuille
 * de style qui suit le système, toute seule, `@media` à l'appui. Les deux
 * autres posent `data-theme` sur `<html>` et le mémorisent.
 *
 * Trois choses doivent rester d'accord, et c'est là que ça casse :
 *
 * 1. `index.html` porte un script qui relit le choix **avant le premier
 *    rendu**. Sans lui, quelqu'un qui a forcé le sombre verrait la page
 *    blanche le temps que React démarre — un éclair blanc en pleine nuit,
 *    exactement ce qu'un thème sombre sert à éviter.
 * 2. `theme.css` déclare le jeu sombre deux fois, pour le système et pour le
 *    choix explicite.
 * 3. La balise `theme-color`, qui teinte la barre du navigateur sur iOS.
 *
 * `Scripts/check_theme.py` les compare entre elles.
 */

export const themeChoices = ["auto", "light", "dark"] as const;

export type ThemeChoice = (typeof themeChoices)[number];

/** Partagée avec le script de `index.html`, qui la recopie en clair. */
export const themeStorageKey = "plum.theme";

/**
 * La teinte de la barre du navigateur, par thème résolu.
 *
 * Le prune de la marque en clair ; son fond de dégradé en sombre — la même
 * couleur de marque, mais celle qui ne vient pas éclairer le haut d'un écran
 * qu'on a choisi sombre.
 */
const browserTint: Record<"light" | "dark", string> = {
  light: "#6b2d5c",
  dark: "#40183a",
};

export function isThemeChoice(value: unknown): value is ThemeChoice {
  return (
    typeof value === "string" &&
    (themeChoices as readonly string[]).includes(value)
  );
}

/** Ce que le stockage retient, ou « auto » s'il ne retient rien de valide. */
export function storedTheme(): ThemeChoice {
  try {
    const stored = window.localStorage.getItem(themeStorageKey);
    return isThemeChoice(stored) ? stored : "auto";
  } catch {
    // Navigation privée, cookies bloqués, iframe cloisonnée : `localStorage`
    // ne renvoie pas une valeur vide, il *lève*. Le thème automatique reste
    // bon, donc il n'y a rien à signaler à qui que ce soit.
    return "auto";
  }
}

/** Ce que le système demande, quand le choix est « auto ». */
export function systemTheme(): "light" | "dark" {
  return typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function resolveTheme(choice: ThemeChoice): "light" | "dark" {
  return choice === "auto" ? systemTheme() : choice;
}

/**
 * Pose le choix sur le document.
 *
 * « auto » *retire* l'attribut au lieu d'y écrire la valeur résolue : c'est ce
 * qui laisse la requête média reprendre la main, y compris quand le système
 * bascule pendant que la page est ouverte.
 */
function applyToDocument(choice: ThemeChoice): void {
  const root = document.documentElement;
  if (choice === "auto") {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = choice;
  }

  const tint = browserTint[resolveTheme(choice)];
  for (const meta of document.querySelectorAll<HTMLMetaElement>(
    'meta[name="theme-color"]',
  )) {
    // Les deux balises du gabarit portent un `media` : elles servent le cas
    // sans JavaScript. Dès qu'on décide ici, ce `media` deviendrait un second
    // avis contradictoire, donc il saute.
    meta.removeAttribute("media");
    meta.content = tint;
  }
}

let current: ThemeChoice = "auto";
let started = false;
const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  if (!started) {
    started = true;
    current = storedTheme();
    applyToDocument(current);

    // Le système peut basculer pendant que la page est ouverte — un coucher de
    // soleil suffit. En « auto » la feuille de style suit toute seule, mais la
    // balise `theme-color`, elle, ne le sait pas.
    window
      .matchMedia("(prefers-color-scheme: dark)")
      .addEventListener("change", () => {
        if (current === "auto") applyToDocument("auto");
      });

    // Deux onglets ouverts : celui qu'on ne regarde pas doit suivre.
    window.addEventListener("storage", (event) => {
      if (event.key !== themeStorageKey) return;
      current = storedTheme();
      applyToDocument(current);
      notify();
    });
  }

  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setTheme(choice: ThemeChoice): void {
  current = choice;
  applyToDocument(choice);
  try {
    // « auto » s'efface plutôt que de s'écrire : rien à retenir, et un jour où
    // la valeur par défaut changerait, les gens qui ne l'ont jamais touchée
    // suivraient.
    if (choice === "auto") {
      window.localStorage.removeItem(themeStorageKey);
    } else {
      window.localStorage.setItem(themeStorageKey, choice);
    }
  } catch {
    // Le thème tient jusqu'à la fin de la visite ; il ne survivra pas au
    // rechargement. Mieux qu'un bouton qui ne fait rien.
  }
  notify();
}

/**
 * Le choix courant, lisible pendant le rendu serveur.
 *
 * Le site est pré-rendu : au moment où les douze pages sont fabriquées,
 * personne n'a encore de préférence. Le rendu serveur dit donc « auto », le
 * navigateur hydrate le même HTML, puis `useSyncExternalStore` relit la vraie
 * valeur et rafraîchit la pastille. Lire `localStorage` directement pendant le
 * rendu donnerait deux arbres différents et une hydratation cassée.
 */
export function useTheme(): ThemeChoice {
  return useSyncExternalStore(
    subscribe,
    () => current,
    () => "auto" as ThemeChoice,
  );
}
