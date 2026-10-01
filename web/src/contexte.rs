//! La langue courante, et le texte qui va avec.
//!
//! La langue vit dans l'adresse plutôt que dans un état, pour que chaque
//! version ait la sienne : partageable, mettable en favori, indexable une
//! langue à la fois. Ça veut dire aussi qu'un rechargement garde la langue
//! choisie sans rien stocker sur la machine de personne.
//!
//! Le texte passe par le contexte avec elle : il est désérialisé une fois par
//! rendu de page, et non une fois par composant qui en a besoin.

use std::rc::Rc;

use yew::prelude::*;

use crate::i18n::{copy, Copy, Language};

#[derive(Clone, PartialEq)]
pub struct Texte {
    pub language: Language,
    pub copy: Rc<Copy>,
}

impl Texte {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            copy: Rc::new(copy(language)),
        }
    }
}

/// Le texte et la langue du rendu en cours.
///
/// Panique hors d'un `ContextProvider`, et c'est voulu : un composant du site
/// qui se retrouverait sans langue rendrait du vide, ce qui se verrait moins
/// vite qu'un plantage au premier essai.
#[hook]
pub fn use_texte() -> Texte {
    use_context::<Texte>().expect("un composant du site rendu hors de sa langue")
}
