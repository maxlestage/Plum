//! Chaque mot du site, dans une seule forme.
//!
//! Pas de bibliothèque d'internationalisation, et la raison n'a pas changé en
//! passant de TypeScript à Rust : pour trois langues et quatre pages, un
//! dictionnaire typé achète la seule garantie qui compte — `Copy` a une forme
//! exacte, donc une clé qui manque à l'espagnol est une erreur, pas un blanc
//! découvert par un visiteur.
//!
//! Ce qui *a* changé : le texte vit désormais dans trois fichiers JSON plutôt
//! que dans trois modules. Ce n'est pas un repli, c'est ce qui rend le
//! portage fidèle — les trois fichiers ont été produits en évaluant les
//! modules TypeScript d'origine, donc aucun mot n'est passé par mes doigts.
//! Et ils restent relisibles par quelqu'un qui traduit sans lire de Rust.
//!
//! La vérification de forme se déplace de la compilation vers le démarrage,
//! avec une nuance qui la sauve : le binaire de pré-rendu désérialise les
//! trois langues au moment de construire le site. Une clé manquante fait donc
//! échouer la construction, pas la visite.

use serde::Deserialize;

pub mod routes;

/// Le français d'abord, et c'est le repli pour un visiteur dont le navigateur
/// demande une langue qu'on n'a pas. Pas une préférence nationale :
/// l'application elle-même est en français, donc envoyer un visiteur sans
/// correspondance vers l'anglais promettrait un produit anglais qui n'existe
/// pas.
pub const LANGUAGES: [Language; 3] = [Language::Fr, Language::En, Language::Es];

pub const FALLBACK_LANGUAGE: Language = Language::Fr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Fr,
    En,
    Es,
}

impl Language {
    pub const fn code(self) -> &'static str {
        match self {
            Language::Fr => "fr",
            Language::En => "en",
            Language::Es => "es",
        }
    }

    /// Ce que la bascule annonce, dans les mots de chaque langue.
    pub const fn endonym(self) -> &'static str {
        match self {
            Language::Fr => "Français",
            Language::En => "English",
            Language::Es => "Español",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fr" => Some(Language::Fr),
            "en" => Some(Language::En),
            "es" => Some(Language::Es),
            _ => None,
        }
    }
}

/// Choisit parmi ce que le navigateur demande, le plus souhaité d'abord.
///
/// La correspondance se fait sur la sous-étiquette principale, pour que
/// `es-419`, `es-MX` et `es` tombent tous sur l'espagnol — l'autre option
/// étant qu'un visiteur mexicain reçoive du français parce que la région n'a
/// pas correspondu.
pub fn detect_language<'a>(preferences: impl IntoIterator<Item = &'a str>) -> Language {
    for preference in preferences {
        let primary = preference
            .split('-')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if let Some(language) = Language::parse(&primary) {
            return language;
        }
    }
    FALLBACK_LANGUAGE
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Entry {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Themes {
    pub auto: String,
    pub light: String,
    pub dark: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Nav {
    pub home: String,
    pub terms: String,
    pub privacy: String,
    pub help: String,
    pub language_label: String,
    pub skip_to_content: String,
    pub theme_label: String,
    /// Ce qu'un lecteur d'écran annonce pour le bouton du menu.
    pub menu_label: String,
    pub themes: Themes,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hero {
    pub headline_top: String,
    pub headline_bottom: String,
    pub tagline: String,
    pub availability_cta: String,
    pub question_cta: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    pub eyebrow: String,
    pub title: String,
    pub body: String,
    pub no_mailing_list: String,
    /// L'application elle-même n'existe qu'en français. Le dire sur les pages
    /// anglaise et espagnole est la différence entre traduire un site et
    /// tromper les gens qui lisent la traduction.
    pub app_language_notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub heading: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub title: String,
    pub sections: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    // Les titres et les descriptions ne sont pas ici : `site.json` les porte,
    // parce que le pré-rendu a besoin des mêmes valeurs. Deux copies d'un
    // titre, c'est un titre qui se périme.
    pub nav: Nav,
    pub hero: Hero,
    pub steps_eyebrow: String,
    /// Trois, et la longueur est fixée exprès : le tableau oblige les trois
    /// langues à bouger ensemble.
    pub steps: [Entry; 3],
    pub principles_eyebrow: String,
    /// Six, pour la même raison. Une page traduite à laquelle il manque un
    /// principe échouerait au pré-rendu plutôt que de se publier en silence.
    pub principles: [Entry; 6],
    pub availability: Availability,
    pub footer_tagline: String,
    pub draft: Draft,
    pub terms: Document,
    pub privacy: Document,
    pub help: Document,
}

const FR: &str = include_str!("fr.json");
const EN: &str = include_str!("en.json");
const ES: &str = include_str!("es.json");

/// Le texte d'une langue.
///
/// Désérialisé à chaque appel plutôt que mis en cache : les composants en
/// prennent un `Rc` par le contexte, donc l'appel n'a lieu qu'une fois par
/// rendu de page, et un verrou global pour trois objets de huit kilo-octets
/// coûterait plus de complexité qu'il n'épargne de travail.
pub fn copy(language: Language) -> Copy {
    let brut = match language {
        Language::Fr => FR,
        Language::En => EN,
        Language::Es => ES,
    };
    serde_json::from_str(brut)
        .unwrap_or_else(|erreur| panic!("{}.json ne correspond plus à `Copy` : {erreur}", language.code()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les trois langues ont la forme exacte. C'est le contrôle que le tuple
    /// TypeScript faisait à la compilation ; il se fait ici, et le pré-rendu
    /// l'exécute à chaque construction du site.
    #[test]
    fn les_trois_langues_ont_la_meme_forme() {
        for language in LANGUAGES {
            let texte = copy(language);
            assert_eq!(texte.steps.len(), 3, "{}", language.code());
            assert_eq!(texte.principles.len(), 6, "{}", language.code());
            assert!(!texte.nav.home.is_empty(), "{}", language.code());
            assert!(!texte.terms.sections.is_empty(), "{}", language.code());
            assert!(!texte.privacy.sections.is_empty(), "{}", language.code());
            assert!(!texte.help.sections.is_empty(), "{}", language.code());
        }
    }

    /// Seul le français se passe de l'avertissement sur la langue de
    /// l'application : c'est la seule traduction qui ne promet rien d'autre
    /// que ce que l'application fait.
    #[test]
    fn les_traductions_disent_que_lapplication_est_en_francais() {
        assert!(copy(Language::Fr).availability.app_language_notice.is_none());
        for language in [Language::En, Language::Es] {
            assert!(
                copy(language).availability.app_language_notice.is_some(),
                "{} ne prévient pas que l'application est en français",
                language.code()
            );
        }
    }

    #[test]
    fn la_langue_vient_de_la_sous_etiquette_principale() {
        assert_eq!(detect_language(["es-419", "fr"]), Language::Es);
        assert_eq!(detect_language(["ES-mx"]), Language::Es);
        assert_eq!(detect_language(["de", "en-GB"]), Language::En);
        assert_eq!(detect_language(["de"]), FALLBACK_LANGUAGE);
        assert_eq!(detect_language([]), FALLBACK_LANGUAGE);
    }
}
