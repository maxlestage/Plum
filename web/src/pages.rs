//! Les quatre pages : l'accueil, et les trois documents.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::app::Route;
use crate::components::marque::PlumMark;
use crate::contexte::use_texte;
use crate::i18n::routes::Page;
use crate::i18n::Entry;

#[function_component]
pub fn Home() -> Html {
    let texte = use_texte();
    let language = texte.language;
    let copy = texte.copy.clone();

    html! {
        <>
            <section class="section" style="padding-top:72px">
                <div class="shell">
                    <div class="prose">
                        <PlumMark size={72} />
                        <h1 style="font-size:clamp(2.2rem, 8vw, 3.4rem);margin:20px 0 12px">
                            { copy.hero.headline_top.clone() }
                            <br />
                            { copy.hero.headline_bottom.clone() }
                        </h1>
                        <p class="muted" style="font-size:1.15rem;margin:0 0 28px">
                            { copy.hero.tagline.clone() }
                        </p>
                        <div style="display:flex;flex-wrap:wrap;gap:12px">
                            <a class="button" href="#availability">
                                { copy.hero.availability_cta.clone() }
                            </a>
                            <Link<Route>
                                classes="button button--quiet"
                                to={Route::depuis(language, Page::Help)}
                            >{ copy.hero.question_cta.clone() }</Link<Route>>
                        </div>
                    </div>
                </div>
            </section>

            <section class="section">
                <div class="shell">
                    <p class="eyebrow">{ copy.steps_eyebrow.clone() }</p>
                    <div class="grid grid--three">
                        { for copy.steps.iter().enumerate().map(|(index, etape)| html! {
                            <article class="card">
                                <p
                                    aria-hidden="true"
                                    style="margin:0;font-weight:700;font-size:1.6rem;\
                                           background:var(--warm-gradient);\
                                           -webkit-background-clip:text;background-clip:text;\
                                           color:transparent"
                                >{ index + 1 }</p>
                                <h2 style="font-size:1.15rem;margin:8px 0">
                                    { etape.title.clone() }
                                </h2>
                                <p class="muted" style="margin:0">{ etape.body.clone() }</p>
                            </article>
                        }) }
                    </div>
                </div>
            </section>

            <section class="section">
                <div class="shell">
                    <p class="eyebrow">{ copy.principles_eyebrow.clone() }</p>
                    <div class="grid grid--two">
                        { for copy.principles.iter().map(|principe| html! {
                            <article class="card">
                                <h2 style="font-size:1.15rem;margin:0 0 8px">
                                    { principe.title.clone() }
                                </h2>
                                <p class="muted" style="margin:0">{ principe.body.clone() }</p>
                            </article>
                        }) }
                    </div>
                </div>
            </section>

            <section class="section" id="availability">
                <div class="shell prose">
                    <p class="eyebrow">{ copy.availability.eyebrow.clone() }</p>
                    <h2 style="font-size:1.6rem;margin-bottom:12px">
                        { copy.availability.title.clone() }
                    </h2>
                    <p class="muted">{ copy.availability.body.clone() }</p>
                    <p class="muted">{ copy.availability.no_mailing_list.clone() }</p>
                    // Montré seulement là où c'est vrai : l'application est en
                    // français, donc les pages traduites le disent plutôt que
                    // de laisser quelqu'un le découvrir après l'avoir
                    // téléchargée.
                    if let Some(avis) = copy.availability.app_language_notice.clone() {
                        <p class="muted"><strong>{ avis }</strong></p>
                    }
                </div>
            </section>
        </>
    }
}

/// Ces pages décrivent ce que l'application fait vraiment — cette partie-là
/// est juste, parce qu'elle a été écrite depuis le code. Ce qu'elles ne sont
/// pas, c'est un avis juridique, et une application de rencontres est un
/// mauvais endroit pour improviser : les seuls critères de recherche
/// permettent déjà de déduire une orientation sexuelle, que le RGPD range
/// parmi les catégories particulières. D'où le bandeau, qui reste jusqu'à ce
/// qu'un juriste les ait relues — dans les trois langues, puisqu'un brouillon
/// traduit reste un brouillon.
#[function_component]
fn Brouillon() -> Html {
    let texte = use_texte();

    html! {
        <div
            class="card"
            style="border-color:var(--apricot);\
                   background:color-mix(in srgb, var(--apricot) 12%, var(--surface));\
                   margin-bottom:28px"
        >
            <strong>{ texte.copy.draft.heading.clone() }</strong>
            <p style="margin:8px 0 0">{ texte.copy.draft.body.clone() }</p>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct DocumentProps {
    title: String,
    sections: Vec<Entry>,
}

#[function_component]
fn Document(props: &DocumentProps) -> Html {
    html! {
        <section class="section">
            <div class="shell prose">
                <h1 style="font-size:clamp(1.8rem, 6vw, 2.4rem);margin-bottom:20px">
                    { props.title.clone() }
                </h1>
                <Brouillon />
                { for props.sections.iter().map(|section| html! {
                    <div>
                        <h2 style="font-size:1.2rem;margin-top:28px">
                            { section.title.clone() }
                        </h2>
                        <p class="muted">{ section.body.clone() }</p>
                    </div>
                }) }
            </div>
        </section>
    }
}

#[function_component]
pub fn Terms() -> Html {
    let texte = use_texte();
    html! {
        <Document
            title={texte.copy.terms.title.clone()}
            sections={texte.copy.terms.sections.clone()}
        />
    }
}

#[function_component]
pub fn Privacy() -> Html {
    let texte = use_texte();
    html! {
        <Document
            title={texte.copy.privacy.title.clone()}
            sections={texte.copy.privacy.sections.clone()}
        />
    }
}

#[function_component]
pub fn Help() -> Html {
    let texte = use_texte();
    html! {
        <Document
            title={texte.copy.help.title.clone()}
            sections={texte.copy.help.sections.clone()}
        />
    }
}
