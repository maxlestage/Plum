//! Le thème : automatique, clair, sombre.
//!
//! « Automatique » est la valeur par défaut et n'écrit rien — c'est la feuille
//! de style qui suit le système, toute seule, `@media` à l'appui. Les deux
//! autres posent `data-theme` sur `<html>` et le mémorisent.
//!
//! Trois choses doivent rester d'accord, et c'est là que ça casse :
//!
//! 1. `index.html` porte un script qui relit le choix **avant le premier
//!    rendu**. Il reste en JavaScript, et ce n'est pas un oubli : le wasm
//!    arrive bien après le premier pixel, donc quelqu'un qui a forcé le
//!    sombre verrait la page blanche le temps du téléchargement — un éclair
//!    blanc en pleine nuit, exactement ce qu'un thème sombre sert à éviter.
//!    C'est même plus vrai qu'avec React : le wasm pèse davantage que le
//!    bundle qu'il remplace.
//! 2. `theme.css` déclare le jeu sombre deux fois, pour le système et pour le
//!    choix explicite.
//! 3. La balise `theme-color`, qui teinte la barre du navigateur sur iOS.
//!
//! `Scripts/check_theme.py` les compare entre elles.

/// Partagée avec le script de `index.html`, qui la recopie en clair.
pub const STORAGE_KEY: &str = "plum.theme";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    Auto,
    Light,
    Dark,
}

/// Automatique, clair, sombre — dans cet ordre, et « automatique » d'abord
/// parce que c'est l'état par défaut et celui auquel on revient.
pub const THEME_CHOICES: [ThemeChoice; 3] =
    [ThemeChoice::Auto, ThemeChoice::Light, ThemeChoice::Dark];

impl ThemeChoice {
    pub const fn key(self) -> &'static str {
        match self {
            ThemeChoice::Auto => "auto",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(ThemeChoice::Auto),
            "light" => Some(ThemeChoice::Light),
            "dark" => Some(ThemeChoice::Dark),
            _ => None,
        }
    }
}

/// La teinte de la barre du navigateur, par thème résolu.
///
/// Le prune de la marque en clair ; son fond de dégradé en sombre — la même
/// couleur de marque, mais celle qui ne vient pas éclairer le haut d'un écran
/// qu'on a choisi sombre.
pub const fn browser_tint(resolved: Resolved) -> &'static str {
    match resolved {
        Resolved::Light => "#6b2d5c",
        Resolved::Dark => "#40183a",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolved {
    Light,
    Dark,
}

pub fn resolve(choice: ThemeChoice, system: Resolved) -> Resolved {
    match choice {
        ThemeChoice::Auto => system,
        ThemeChoice::Light => Resolved::Light,
        ThemeChoice::Dark => Resolved::Dark,
    }
}

#[cfg(target_arch = "wasm32")]
mod navigateur {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gloo_events::EventListener;
    use wasm_bindgen::JsCast;

    use super::{browser_tint, resolve, Resolved, ThemeChoice, STORAGE_KEY};

    thread_local! {
        static ETAT: RefCell<Etat> = RefCell::new(Etat::default());
    }

    #[derive(Default)]
    struct Etat {
        courant: Option<ThemeChoice>,
        /// Les écoutes vivent aussi longtemps que la page : on les garde pour
        /// qu'elles ne soient pas détruites à la sortie de `demarrer`.
        _ecoutes: Vec<EventListener>,
        auditeurs: Vec<Rc<dyn Fn()>>,
    }

    /// Ce que le système demande, quand le choix est « auto ».
    pub fn systeme() -> Resolved {
        web_sys::window()
            .and_then(|f| f.match_media("(prefers-color-scheme: dark)").ok().flatten())
            .map(|liste| liste.matches())
            .map(|sombre| if sombre { Resolved::Dark } else { Resolved::Light })
            .unwrap_or(Resolved::Light)
    }

    /// Ce que le stockage retient, ou « auto » s'il ne retient rien de valide.
    ///
    /// En navigation privée, cookies bloqués, iframe cloisonnée,
    /// `localStorage` ne rend pas une valeur vide : il *lève*. Le thème
    /// automatique reste bon, donc il n'y a rien à signaler à qui que ce soit.
    pub fn stocke() -> ThemeChoice {
        web_sys::window()
            .and_then(|f| f.local_storage().ok().flatten())
            .and_then(|magasin| magasin.get_item(STORAGE_KEY).ok().flatten())
            .and_then(|valeur| ThemeChoice::parse(&valeur))
            .unwrap_or(ThemeChoice::Auto)
    }

    /// Pose le choix sur le document.
    ///
    /// « auto » *retire* l'attribut au lieu d'y écrire la valeur résolue :
    /// c'est ce qui laisse la requête média reprendre la main, y compris quand
    /// le système bascule pendant que la page est ouverte.
    pub fn appliquer(choice: ThemeChoice) {
        let Some(document) = web_sys::window().and_then(|f| f.document()) else {
            return;
        };
        if let Some(racine) = document.document_element() {
            if choice == ThemeChoice::Auto {
                let _ = racine.remove_attribute("data-theme");
            } else {
                let _ = racine.set_attribute("data-theme", choice.key());
            }
        }

        let teinte = browser_tint(resolve(choice, systeme()));
        let Ok(balises) = document.query_selector_all("meta[name=\"theme-color\"]") else {
            return;
        };
        for index in 0..balises.length() {
            let Some(noeud) = balises.item(index) else { continue };
            let Ok(meta) = noeud.dyn_into::<web_sys::HtmlMetaElement>() else {
                continue;
            };
            // Les deux balises du gabarit portent un `media` : elles servent
            // le cas sans JavaScript. Dès qu'on décide ici, ce `media`
            // deviendrait un second avis contradictoire, donc il saute.
            let _ = meta.remove_attribute("media");
            meta.set_content(teinte);
        }
    }

    /// Le choix courant, en démarrant l'écoute à la première demande.
    pub fn courant_ou_demarrer(auditeur: Rc<dyn Fn()>) -> ThemeChoice {
        ETAT.with(|etat| {
            let mut etat = etat.borrow_mut();
            etat.auditeurs.push(auditeur);

            if let Some(courant) = etat.courant {
                return courant;
            }

            let courant = stocke();
            etat.courant = Some(courant);
            appliquer(courant);

            if let Some(fenetre) = web_sys::window() {
                // Le système peut basculer pendant que la page est ouverte —
                // un coucher de soleil suffit. En « auto » la feuille de style
                // suit toute seule, mais la balise `theme-color`, elle, ne le
                // sait pas.
                if let Ok(Some(liste)) = fenetre.match_media("(prefers-color-scheme: dark)") {
                    etat._ecoutes.push(EventListener::new(&liste, "change", |_| {
                        let courant = ETAT.with(|e| e.borrow().courant).unwrap_or(ThemeChoice::Auto);
                        if courant == ThemeChoice::Auto {
                            appliquer(ThemeChoice::Auto);
                        }
                    }));
                }

                // Deux onglets ouverts : celui qu'on ne regarde pas doit suivre.
                etat._ecoutes.push(EventListener::new(&fenetre, "storage", |evenement| {
                    let clef = evenement
                        .dyn_ref::<web_sys::StorageEvent>()
                        .and_then(|e| e.key());
                    if clef.as_deref() != Some(STORAGE_KEY) {
                        return;
                    }
                    let relu = stocke();
                    ETAT.with(|e| e.borrow_mut().courant = Some(relu));
                    appliquer(relu);
                    prevenir();
                }));
            }

            courant
        })
    }

    fn prevenir() {
        let auditeurs: Vec<Rc<dyn Fn()>> =
            ETAT.with(|etat| etat.borrow().auditeurs.clone());
        for auditeur in auditeurs {
            auditeur();
        }
    }

    /// La valeur courante, sans rien démarrer : sert au rafraîchissement
    /// déclenché par une écoute déjà en place.
    pub fn lu() -> ThemeChoice {
        ETAT.with(|etat| etat.borrow().courant).unwrap_or(ThemeChoice::Auto)
    }

    pub fn oublier(auditeur: &Rc<dyn Fn()>) {
        ETAT.with(|etat| {
            etat.borrow_mut()
                .auditeurs
                .retain(|inscrit| !Rc::ptr_eq(inscrit, auditeur))
        });
    }

    pub fn poser(choice: ThemeChoice) {
        ETAT.with(|etat| etat.borrow_mut().courant = Some(choice));
        appliquer(choice);

        if let Some(magasin) = web_sys::window().and_then(|f| f.local_storage().ok().flatten()) {
            // « auto » s'efface plutôt que de s'écrire : rien à retenir, et un
            // jour où la valeur par défaut changerait, les gens qui ne l'ont
            // jamais touchée suivraient.
            let _ = if choice == ThemeChoice::Auto {
                magasin.remove_item(STORAGE_KEY)
            } else {
                magasin.set_item(STORAGE_KEY, choice.key())
            };
        }
        // Le thème tient jusqu'à la fin de la visite même si le stockage a
        // levé ; il ne survivra pas au rechargement. Mieux qu'un bouton qui
        // ne fait rien.
        prevenir();
    }
}

#[cfg(target_arch = "wasm32")]
pub use navigateur::{
    courant_ou_demarrer as abonner, lu, oublier as desabonner, poser as set_theme,
    systeme as system_theme,
};

/// Hors du navigateur — c'est-à-dire pendant le pré-rendu — il n'y a ni
/// stockage ni requête média. Le rendu dit donc « auto », le navigateur
/// hydrate le même HTML, puis relit la vraie valeur et rafraîchit la pastille.
/// Lire une préférence pendant le pré-rendu donnerait deux arbres différents
/// et une hydratation cassée.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_theme(_choice: ThemeChoice) {}

#[cfg(not(target_arch = "wasm32"))]
pub fn system_theme() -> Resolved {
    Resolved::Light
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_suit_le_systeme_et_les_autres_non() {
        assert_eq!(resolve(ThemeChoice::Auto, Resolved::Dark), Resolved::Dark);
        assert_eq!(resolve(ThemeChoice::Auto, Resolved::Light), Resolved::Light);
        assert_eq!(resolve(ThemeChoice::Light, Resolved::Dark), Resolved::Light);
        assert_eq!(resolve(ThemeChoice::Dark, Resolved::Light), Resolved::Dark);
    }

    #[test]
    fn les_cles_font_laller_retour() {
        for choix in THEME_CHOICES {
            assert_eq!(ThemeChoice::parse(choix.key()), Some(choix));
        }
        assert_eq!(ThemeChoice::parse("sombre"), None);
    }

    /// Les deux teintes sont celles que `check_theme.py` compare au CSS et au
    /// gabarit. Écrites en minuscules des deux côtés : le contrôle les
    /// compare caractère par caractère.
    #[test]
    fn les_teintes_sont_celles_de_la_marque() {
        assert_eq!(browser_tint(Resolved::Light), "#6b2d5c");
        assert_eq!(browser_tint(Resolved::Dark), "#40183a");
    }
}
