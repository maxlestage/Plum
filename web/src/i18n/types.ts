/**
 * Every string on the site, in one shape.
 *
 * No i18n library on purpose. For three languages and a handful of pages, a
 * typed dictionary buys the one guarantee that matters: `Copy` is an exact
 * shape, so a key missing from Spanish is a build error rather than a blank
 * space discovered by a visitor. A runtime lookup with a string key would
 * happily render nothing.
 */

export const languages = ["fr", "en", "es"] as const;

export type Language = (typeof languages)[number];

/** What the switcher shows, in each language's own words. */
export const languageNames: Record<Language, string> = {
  fr: "Français",
  en: "English",
  es: "Español",
};

export interface Entry {
  title: string;
  body: string;
}

export interface Copy {
  // Titles and descriptions are not here: `site.json` holds them, because the
  // build-time template generator needs the same values and cannot read
  // TypeScript. Two copies of a title is one title that goes stale.
  nav: {
    home: string;
    terms: string;
    privacy: string;
    help: string;
    languageLabel: string;
    skipToContent: string;
    themeLabel: string;
    /** Automatique, clair, sombre — les trois états de la bascule de thème. */
    themes: Record<"auto" | "light" | "dark", string>;
  };

  hero: {
    headlineTop: string;
    headlineBottom: string;
    tagline: string;
    availabilityCta: string;
    questionCta: string;
  };

  stepsEyebrow: string;
  steps: [Entry, Entry, Entry];

  principlesEyebrow: string;
  // Six, et la longueur est fixée exprès : le tuple oblige les trois langues
  // à bouger ensemble. Une page traduite à laquelle il manque un principe se
  // publierait sans que rien ne le signale.
  principles: [Entry, Entry, Entry, Entry, Entry, Entry];

  availability: {
    eyebrow: string;
    title: string;
    body: string;
    noMailingList: string;
    /**
     * The app itself is French-only. Saying so on the English and Spanish
     * pages is the difference between translating a site and misleading the
     * people who read the translation.
     */
    appLanguageNotice: string | null;
  };

  footerTagline: string;

  draft: {
    heading: string;
    body: string;
  };

  terms: {
    title: string;
    sections: Entry[];
  };

  privacy: {
    title: string;
    sections: Entry[];
  };

  help: {
    title: string;
    sections: Entry[];
  };
}
