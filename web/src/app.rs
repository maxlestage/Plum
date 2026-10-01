//! Le routeur, et ce qu'il résout.

use yew::prelude::*;
use yew_router::history::{AnyHistory, History, MemoryHistory};
use yew_router::prelude::*;

use crate::components::page::{DocumentHead, Layout};
use crate::contexte::Texte;
use crate::i18n::routes::{page_for_slug, path_for, Page};
#[cfg(target_arch = "wasm32")]
use crate::i18n::detect_language;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::FALLBACK_LANGUAGE;
use crate::i18n::Language;
use crate::pages::{Help, Home, Privacy, Terms};

/// Les adresses du site.
///
/// La langue et le slug restent des chaînes : le routeur ne connaît pas les
/// trois langues ni les douze slugs, et c'est `LocalisedRoute` qui les valide.
/// Les déclarer en types fermés obligerait à écrire douze variantes, et une
/// adresse inconnue tomberait en « non trouvée » au lieu d'être renvoyée vers
/// la langue qu'on avait demandée.
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Racine,
    #[at("/:language/")]
    Langue { language: String },
    #[at("/:language/:slug/")]
    Page { language: String, slug: String },
    #[not_found]
    #[at("/404")]
    Inconnue,
}

impl Route {
    /// L'adresse d'une page dans une langue, telle que le routeur la reconnaît.
    ///
    /// Passe par `path_for` plutôt que de construire la variante à la main :
    /// une seule table de slugs, donc les liens du site et les `hreflang` du
    /// pré-rendu ne peuvent pas diverger.
    pub fn depuis(language: Language, page: Page) -> Self {
        let chemin = path_for(language, page);
        let morceaux: Vec<&str> = chemin.trim_matches('/').split('/').collect();
        match morceaux.as_slice() {
            [language] => Route::Langue {
                language: (*language).to_string(),
            },
            [language, slug] => Route::Page {
                language: (*language).to_string(),
                slug: (*slug).to_string(),
            },
            _ => Route::Racine,
        }
    }
}

/// La langue que le navigateur demande. Hors navigateur il n'y a personne à
/// qui demander : le français répond, et c'est le repli de toute façon.
fn langue_du_visiteur() -> Language {
    #[cfg(target_arch = "wasm32")]
    {
        let langues: Vec<String> = web_sys::window()
            .map(|fenetre| fenetre.navigator().languages())
            .map(|liste| {
                liste
                    .iter()
                    .filter_map(|valeur| valeur.as_string())
                    .collect()
            })
            .unwrap_or_default();
        detect_language(langues.iter().map(String::as_str))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        FALLBACK_LANGUAGE
    }
}

#[derive(Properties, PartialEq)]
struct LocaliseProps {
    language: String,
    slug: String,
}

/// Résout `/:language/:slug/`.
#[function_component]
fn Localise(props: &LocaliseProps) -> Html {
    let navigateur = use_navigator();

    let Some(language) = Language::parse(&props.language) else {
        // Une langue inconnue : on envoie vers celle que le navigateur
        // demande, pas vers celle de l'adresse — elle ne veut rien dire.
        return html! {
            <Redirect<Route> to={Route::depuis(langue_du_visiteur(), Page::Home)} />
        };
    };

    let Some(page) = page_for_slug(language, &props.slug) else {
        // Un slug inconnu dans une langue connue : on l'emmène à l'accueil de
        // *cette* langue, pas de celle qu'on a détectée — c'est cette langue
        // qu'il a demandée.
        return html! { <Redirect<Route> to={Route::depuis(language, Page::Home)} /> };
    };

    let _ = navigateur;
    let texte = Texte::new(language);

    html! {
        <ContextProvider<Texte> context={texte}>
            <DocumentHead page={page} />
            <Layout page={page}>
                { match page {
                    Page::Home => html! { <Home /> },
                    Page::Terms => html! { <Terms /> },
                    Page::Privacy => html! { <Privacy /> },
                    Page::Help => html! { <Help /> },
                } }
            </Layout>
        </ContextProvider<Texte>>
    }
}

#[function_component]
fn Vue() -> Html {
    html! {
        <Switch<Route> render={|route: Route| match route {
            // La racine nue choisit une langue d'après le navigateur, une
            // fois, puis passe la main à une vraie adresse.
            Route::Racine | Route::Inconnue => html! {
                <Redirect<Route> to={Route::depuis(langue_du_visiteur(), Page::Home)} />
            },
            Route::Langue { language } => html! {
                <Localise language={language} slug={String::new()} />
            },
            Route::Page { language, slug } => html! {
                <Localise language={language} slug={slug} />
            },
        }} />
    }
}

/// L'arbre côté navigateur.
#[function_component]
pub fn App() -> Html {
    html! { <BrowserRouter><Vue /></BrowserRouter> }
}

#[derive(Properties, PartialEq)]
pub struct StatiqueProps {
    pub chemin: String,
}

/// Le même arbre, pour une adresse donnée, au moment de la construction.
///
/// `MemoryHistory` plutôt que `BrowserHistory` : il n'y a pas d'historique
/// ici, seulement une adresse. Le reste de l'arbre est identique à celui du
/// navigateur, sans quoi l'hydratation trouverait un DOM qu'elle n'a pas
/// produit.
#[function_component]
pub fn AppStatique(props: &StatiqueProps) -> Html {
    let histoire = AnyHistory::from(MemoryHistory::new());
    histoire.push(props.chemin.clone());
    html! { <Router history={histoire}><Vue /></Router> }
}
