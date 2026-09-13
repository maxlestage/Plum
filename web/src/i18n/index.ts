import { createContext, useContext } from "react";

import { en } from "./en";
import { es } from "./es";
import { fr } from "./fr";
import type { Copy, Language } from "./types";
import { languages } from "./types";

export { languages, languageNames } from "./types";
export type { Copy, Entry, Language } from "./types";

/**
 * French first, and it is the fallback for a visitor whose browser asks for
 * something we do not have. Not national preference: the app itself is in
 * French, so sending an unmatched visitor to English would promise an English
 * product that does not exist.
 */
export const copy: Record<Language, Copy> = { fr, en, es };

export function isLanguage(value: string | undefined): value is Language {
  return (
    value !== undefined && (languages as readonly string[]).includes(value)
  );
}

export const fallbackLanguage: Language = "fr";

/**
 * Picks from what the browser asks for, most-preferred first.
 *
 * Matches on the primary subtag, so `es-419`, `es-MX` and `es` all land on
 * Spanish — the alternative is a Mexican visitor getting French because the
 * region did not match.
 */
export function detectLanguage(
  preferences: readonly string[] = typeof navigator === "undefined"
    ? []
    : (navigator.languages ?? [navigator.language]),
): Language {
  for (const preference of preferences) {
    const primary = preference.toLowerCase().split("-")[0];
    if (isLanguage(primary)) {
      return primary;
    }
  }
  return fallbackLanguage;
}

const LanguageContext = createContext<Language>(fallbackLanguage);

export const LanguageProvider = LanguageContext.Provider;

export function useLanguage(): Language {
  return useContext(LanguageContext);
}

export function useCopy(): Copy {
  return copy[useLanguage()];
}

/** Prefixes a path with the current language: `/es/privacidad`. */
export function useLocalisedPath(): (path: string) => string {
  const language = useLanguage();
  return (path: string) => `/${language}${path === "/" ? "" : path}`;
}
