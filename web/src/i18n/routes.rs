//! Les pages, et le mot que chaque langue emploie pour les désigner.
//!
//! La table vit dans `site.json` parce que le pré-rendu a besoin de la même :
//! les adresses qu'il écrit dans les `canonical` et les `hreflang` doivent
//! être exactement celles que l'application produit à l'exécution.
//!
//! Des slugs traduits plutôt que `/es/privacy` : un lecteur espagnol ne
//! devrait pas avoir à lire l'anglais pour savoir où mène un lien. La clé est
//! ce sur quoi le code route, donc les slugs peuvent changer sans toucher à un
//! composant.

use std::collections::HashMap;

use serde::Deserialize;

use super::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Home,
    Terms,
    Privacy,
    Help,
}

pub const PAGES: [Page; 4] = [Page::Home, Page::Terms, Page::Privacy, Page::Help];

impl Page {
    pub const fn key(self) -> &'static str {
        match self {
            Page::Home => "home",
            Page::Terms => "terms",
            Page::Privacy => "privacy",
            Page::Help => "help",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    pub title: String,
    pub description: String,
    #[serde(rename = "pageTitles")]
    pub page_titles: HashMap<String, Option<String>>,
    #[serde(rename = "imageAlt")]
    pub image_alt: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Site {
    /// L'adresse publique du site, sans barre finale. Elle sert à écrire des
    /// `canonical` et des `hreflang` absolus dans les coquilles pré-rendues :
    /// Google ignore un `hreflang` relatif, ce qui rendrait muettes les
    /// annotations trilingues là où elles comptent — dans le HTML que lisent
    /// les robots. À l'exécution, l'application préfère l'origine réellement
    /// servie, qui est plus juste.
    pub origin: String,
    pub slugs: HashMap<String, HashMap<String, String>>,
    pub meta: HashMap<String, Meta>,
}

const SITE: &str = include_str!("site.json");

pub fn site() -> Site {
    serde_json::from_str(SITE).expect("site.json ne correspond plus à `Site`")
}

/// Toujours avec une barre finale.
///
/// Les pages sont servies comme index de dossier, et `ServeDir` répond à une
/// adresse de dossier sans barre finale par une redirection 307 vers celle qui
/// en a une. Déclarer la forme sans barre dans un `hreflang` ferait pointer
/// chaque traduction vers une adresse qui redirige avant de servir, et
/// laisserait la barre d'adresse en désaccord avec le `canonical` après un
/// rechargement.
pub fn path_for(language: Language, page: Page) -> String {
    let site = site();
    let slug = site
        .slugs
        .get(language.code())
        .and_then(|table| table.get(page.key()))
        .map(String::as_str)
        .unwrap_or_default();

    if slug.is_empty() {
        format!("/{}/", language.code())
    } else {
        format!("/{}/{}/", language.code(), slug)
    }
}

/// À quelle page un slug renvoie, dans une langue donnée.
pub fn page_for_slug(language: Language, slug: &str) -> Option<Page> {
    if slug.is_empty() {
        return Some(Page::Home);
    }
    let site = site();
    let table = site.slugs.get(language.code())?;
    PAGES
        .into_iter()
        .find(|page| table.get(page.key()).map(String::as_str) == Some(slug))
}

/// Le titre complet d'une page : la même règle que le gabarit appliquait, à
/// partir de la même table.
pub fn document_title(language: Language, page: Page) -> String {
    let site = site();
    let meta = site
        .meta
        .get(language.code())
        .expect("site.json décrit les trois langues");
    match meta.page_titles.get(page.key()).and_then(Option::as_deref) {
        Some(section) => format!("{section} — Plum"),
        None => meta.title.clone(),
    }
}

pub fn description(language: Language) -> String {
    site()
        .meta
        .get(language.code())
        .expect("site.json décrit les trois langues")
        .description
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::LANGUAGES;

    #[test]
    fn chaque_adresse_finit_par_une_barre() {
        for language in LANGUAGES {
            for page in PAGES {
                let chemin = path_for(language, page);
                assert!(chemin.ends_with('/'), "{chemin}");
                assert!(chemin.starts_with(&format!("/{}/", language.code())), "{chemin}");
            }
        }
    }

    /// L'aller-retour : l'adresse qu'on écrit est celle qu'on sait relire.
    /// Sans ce contrôle, un slug renommé d'un seul côté donnerait des liens
    /// qui mènent à une redirection vers l'accueil.
    #[test]
    fn un_slug_ecrit_est_un_slug_relu() {
        for language in LANGUAGES {
            for page in PAGES {
                let chemin = path_for(language, page);
                let slug = chemin
                    .trim_start_matches(&format!("/{}/", language.code()))
                    .trim_end_matches('/');
                assert_eq!(page_for_slug(language, slug), Some(page), "{chemin}");
            }
        }
    }

    #[test]
    fn un_slug_inconnu_ne_renvoie_aucune_page() {
        assert_eq!(page_for_slug(Language::Fr, "nexistepas"), None);
        // Et le slug d'une autre langue n'en est pas un dans celle-ci.
        assert_eq!(page_for_slug(Language::Fr, "privacy"), None);
    }

    #[test]
    fn laccueil_porte_le_titre_du_site_et_les_autres_le_leur() {
        assert_eq!(
            document_title(Language::Fr, Page::Home),
            "Plum — Une photo, deux phrases"
        );
        assert_eq!(
            document_title(Language::Fr, Page::Privacy),
            "Confidentialité — Plum"
        );
    }
}
