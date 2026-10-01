//! L'ossature d'une page, et ce qu'elle dit dans l'en-tête du document.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::app::Route;
use crate::contexte::use_texte;
use crate::i18n::routes::Page;

// Ces quatre-là ne servent qu'à l'effet qui retouche la tête du document,
// lequel n'existe qu'au navigateur.
#[cfg(target_arch = "wasm32")]
use crate::i18n::routes::{description, document_title, path_for};
#[cfg(target_arch = "wasm32")]
use crate::i18n::{Language, LANGUAGES};

use super::bascules::HeaderMenu;
use super::marque::PlumMark;

#[derive(Properties, PartialEq)]
pub struct LayoutProps {
    pub page: Page,
    #[prop_or_default]
    pub children: Html,
}

#[function_component]
pub fn Layout(props: &LayoutProps) -> Html {
    let texte = use_texte();
    let language = texte.language;

    html! {
        <>
            <a class="skip-link" href="#main">{ texte.copy.nav.skip_to_content.clone() }</a>

            <header style="border-bottom:1px solid var(--hairline);background:var(--surface)">
                <div class="shell entete">
                    <Link<Route> to={Route::depuis(language, Page::Home)} classes="entete__marque">
                        <PlumMark size={30} />
                        { "Plum" }
                    </Link<Route>>
                    <HeaderMenu page={props.page} />
                </div>
            </header>

            <main id="main">{ props.children.clone() }</main>

            <footer style="border-top:1px solid var(--hairline);padding:40px 0;margin-top:40px">
                <div class="shell">
                    <nav style="display:flex;flex-wrap:wrap;gap:12px 24px;margin-bottom:16px">
                        if props.page != Page::Home {
                            <Link<Route> to={Route::depuis(language, Page::Home)}>
                                { texte.copy.nav.home.clone() }
                            </Link<Route>>
                        }
                        <Link<Route> to={Route::depuis(language, Page::Terms)}>
                            { texte.copy.nav.terms.clone() }
                        </Link<Route>>
                        <Link<Route> to={Route::depuis(language, Page::Privacy)}>
                            { texte.copy.nav.privacy.clone() }
                        </Link<Route>>
                        <Link<Route> to={Route::depuis(language, Page::Help)}>
                            { texte.copy.nav.help.clone() }
                        </Link<Route>>
                    </nav>
                    <p class="muted" style="margin:0;font-size:0.9rem">
                        { texte.copy.footer_tagline.clone() }
                    </p>
                </div>
            </footer>
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct DocumentHeadProps {
    pub page: Page,
}

/// Garde en phase avec la page les parties de `<head>` qui dépendent de la
/// langue.
///
/// Les coquilles pré-rendues portent déjà les bonnes valeurs pour l'adresse
/// qui a été demandée — c'est tout l'intérêt du pré-rendu. Ce composant sert
/// la navigation côté client : après être passé de `/fr/` à `/es/`, le titre,
/// la langue du document et les alternates décriraient encore la page d'où
/// l'on vient.
///
/// Il ne rend rien. Il agit sur le document, ce qui n'a de sens que dans un
/// navigateur : côté pré-rendu, c'est le gabarit qui écrit ces balises.
#[function_component]
pub fn DocumentHead(props: &DocumentHeadProps) -> Html {
    let texte = use_texte();
    let page = props.page;
    let language = texte.language;

    // Le hook est appelé sans condition : Yew exige qu'ils le soient tous, à
    // chaque rendu, dans le même ordre. C'est le corps de l'effet qui se tait
    // hors du navigateur, où il n'y a pas de document à toucher.
    use_effect_with((language, page), move |(language, page)| {
        #[cfg(target_arch = "wasm32")]
        appliquer_la_tete(*language, *page);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (language, page);
        || ()
    });

    html! {}
}

#[cfg(target_arch = "wasm32")]
fn appliquer_la_tete(language: Language, page: Page) {
    use wasm_bindgen::JsCast;

    let Some(fenetre) = web_sys::window() else { return };
    let Some(document) = fenetre.document() else { return };

    let titre = document_title(language, page);
    let resume = description(language);

    if let Some(racine) = document.document_element() {
        let _ = racine.set_attribute("lang", language.code());
    }
    document.set_title(&titre);

    let poser = |selecteur: &str, valeur: &str| {
        if let Ok(Some(element)) = document.query_selector(selecteur) {
            let _ = element.set_attribute("content", valeur);
        }
    };
    poser("meta[name=\"description\"]", &resume);
    poser("meta[property=\"og:title\"]", &titre);
    poser("meta[property=\"og:description\"]", &resume);

    // Les alternates disent à un moteur que ce sont les mêmes pages dans une
    // autre langue plutôt que trois pages qui se concurrencent.
    //
    // *Toutes* partent, pas seulement celles posées ici : la coquille
    // pré-rendue livre son propre jeu pour l'adresse qui a été demandée, et
    // après une navigation côté client celles-là décrivent la page d'où l'on
    // vient. En ajouter à côté laisserait deux jeux contradictoires — ce que
    // le test de navigateur avait justement attrapé.
    if let Ok(precedentes) = document.query_selector_all("link[rel=alternate]") {
        for index in 0..precedentes.length() {
            if let Some(noeud) = precedentes.item(index) {
                if let Some(element) = noeud.dyn_ref::<web_sys::Element>() {
                    element.remove();
                }
            }
        }
    }

    let Some(tete) = document.head() else { return };
    let origine = fenetre.location().origin().unwrap_or_default();

    for entree in LANGUAGES.into_iter().map(Some).chain(std::iter::once(None)) {
        let (hreflang, cible) = match entree {
            Some(language) => (language.code().to_string(), language),
            None => ("x-default".to_string(), Language::Fr),
        };
        let Ok(element) = document.create_element("link") else { continue };
        let Ok(lien) = element.dyn_into::<web_sys::HtmlLinkElement>() else {
            continue;
        };
        lien.set_rel("alternate");
        let _ = lien.set_attribute("hreflang", &hreflang);
        let _ = lien.set_attribute(
            "href",
            &format!("{origine}{}", path_for(cible, page)),
        );
        let _ = lien.set_attribute("data-i18n-alternate", "");
        let _ = tete.append_child(&lien);
    }
}
