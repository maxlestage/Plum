//! Les commandes de l'en-tête : la langue, le thème, et le bouton qui les
//! range derrière lui.

#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use yew::prelude::*;

use crate::contexte::use_texte;
use crate::i18n::routes::{path_for, Page};
use crate::i18n::{Language, LANGUAGES};
use crate::theme::{set_theme, ThemeChoice, THEME_CHOICES};

#[derive(Properties, PartialEq)]
pub struct PageProps {
    pub page: Page,
}

/// Trois codes courts, la langue courante en pastille pleine.
///
/// Des liens, pas un `<select>` : un menu déroulant demande du script pour
/// naviguer et ne donne rien à suivre à un robot. Trois liens s'explorent, se
/// clique-molettent, et fonctionnent pareil sur un téléphone. Ils gardent la
/// page qu'on lisait plutôt que de renvoyer à l'accueil, ce qui est justement
/// ce qui rend un sélecteur agaçant.
///
/// Le code est ce qu'on voit ; le nom complet est ce qu'on entend. « FR » lu à
/// voix haute par un lecteur d'écran ne veut rien dire — `aria-label` porte
/// donc l'endonyme, « Français », et la langue de ce mot est déclarée avec lui,
/// sans quoi une voix française prononcerait « Español » à la française.
#[function_component]
pub fn LanguageSwitcher(props: &PageProps) -> Html {
    let texte = use_texte();
    let page = props.page;

    html! {
        <nav class="bascules" aria-label={texte.copy.nav.language_label.clone()}>
            { for LANGUAGES.into_iter().map(|language| {
                let code = language.code().to_uppercase();
                let nom = language.endonym();

                if language == texte.language {
                    html! {
                        <span
                            class="pastille pastille--active"
                            aria-current="true"
                            aria-label={nom}
                            lang={language.code()}
                        >{ code }</span>
                    }
                } else {
                    html! {
                        <a
                            href={path_for(language, page)}
                            class="pastille"
                            // `hreflang` dit au navigateur et au robot ce
                            // qu'il y a au bout avant qu'ils y aillent ;
                            // `lang` dit comment prononcer l'étiquette.
                            //
                            // Un `<a>` et non le `Link` du routeur : celui-ci
                            // ne porte aucun de ces trois attributs, et les
                            // poser après coup par script les laisserait
                            // absents du HTML pré-rendu — c'est-à-dire
                            // exactement là où `hreflang` sert. Changer de
                            // langue recharge donc le document, ce qui a le
                            // mérite de faire venir du serveur le `lang` et
                            // la tête de la nouvelle langue.
                            hreflang={language.code()}
                            lang={language.code()}
                            aria-label={nom}
                        >{ code }</a>
                    }
                }
            }) }
        </nav>
    }
}

/// Automatique, clair, sombre — dans cet ordre, et « automatique » d'abord
/// parce que c'est l'état par défaut et celui auquel on revient.
///
/// Des boutons, pas des liens : le thème est une préférence d'appareil, pas
/// une adresse. Et des dessins, pas des caractères : ☀ et ☾ sont rendus en
/// émoji couleur par certains systèmes et en glyphe noir par d'autres, ce qui
/// donne une rangée dont un tiers ne ressemble pas au reste. Un SVG en
/// `currentColor` se comporte pareil partout et vire au crème sur la pastille
/// pleine.
///
/// `aria-pressed` plutôt que `aria-current` : trois boutons dont un est
/// enfoncé, c'est ce que c'est. `aria-current` désigne l'endroit où l'on est
/// dans une navigation, et ceci n'est pas une navigation.
#[function_component]
pub fn ThemeSwitcher() -> Html {
    let texte = use_texte();
    let courant = use_theme();

    html! {
        <div class="bascules" role="group" aria-label={texte.copy.nav.theme_label.clone()}>
            { for THEME_CHOICES.into_iter().map(|choix| {
                let etiquette = match choix {
                    ThemeChoice::Auto => texte.copy.nav.themes.auto.clone(),
                    ThemeChoice::Light => texte.copy.nav.themes.light.clone(),
                    ThemeChoice::Dark => texte.copy.nav.themes.dark.clone(),
                };
                let actif = choix == courant;
                let au_clic = Callback::from(move |_| set_theme(choix));

                html! {
                    <button
                        type="button"
                        class={if actif { "pastille pastille--active" } else { "pastille" }}
                        aria-pressed={if actif { "true" } else { "false" }}
                        // Le dessin ne se lit pas à voix haute : l'étiquette
                        // porte le mot.
                        aria-label={etiquette.clone()}
                        title={etiquette}
                        onclick={au_clic}
                    >
                        <Glyphe choix={choix} />
                    </button>
                }
            }) }
        </div>
    }
}

/// Le choix courant, lisible aussi pendant le pré-rendu.
///
/// Au moment où les treize pages sont fabriquées, personne n'a encore de
/// préférence : le rendu dit donc « auto », le navigateur hydrate le même
/// HTML, puis cet effet relit la vraie valeur et rafraîchit la pastille. Lire
/// le stockage pendant le rendu donnerait deux arbres différents et une
/// hydratation cassée.
#[hook]
fn use_theme() -> ThemeChoice {
    let courant = use_state(|| ThemeChoice::Auto);

    {
        let courant = courant.clone();
        use_effect_with((), move |_| {
            #[cfg(target_arch = "wasm32")]
            let menage = {
                let rafraichir: Rc<dyn Fn()> = {
                    let courant = courant.clone();
                    Rc::new(move || courant.set(crate::theme::lu()))
                };
                let initial = crate::theme::abonner(rafraichir.clone());
                courant.set(initial);
                move || crate::theme::desabonner(&rafraichir)
            };
            #[cfg(not(target_arch = "wasm32"))]
            let menage = {
                let _ = courant;
                || ()
            };
            menage
        });
    }

    *courant
}

#[derive(Properties, PartialEq)]
struct GlypheProps {
    choix: ThemeChoice,
}

/// Les trois dessins. Décoratifs : leur sens est déjà dans l'`aria-label` du
/// bouton, et le répéter ferait entendre l'étiquette deux fois.
#[function_component]
fn Glyphe(props: &GlypheProps) -> Html {
    match props.choix {
        ThemeChoice::Light => html! {
            <svg
                viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"
                stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"
            >
                <circle cx="8" cy="8" r="3.1" />
                { for [0, 45, 90, 135, 180, 225, 270, 315].into_iter().map(|angle| html! {
                    <line x1="8" y1="1.6" x2="8" y2="3.2"
                          transform={format!("rotate({angle} 8 8)")} />
                }) }
            </svg>
        },
        // Un croissant d'un seul trait : deux arcs qui se rejoignent.
        // Découper un disque par un second disque donnerait une forme qui
        // disparaît dès que le fond passe au prune plein.
        ThemeChoice::Dark => html! {
            <svg
                viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"
                stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"
            >
                <path d="M13.2 10.1A5.8 5.8 0 0 1 5.9 2.8a5.8 5.8 0 1 0 7.3 7.3Z" />
            </svg>
        },
        // Automatique : un disque à moitié plein, la moitié claire et la
        // moitié sombre du même objet.
        ThemeChoice::Auto => html! {
            <svg
                viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"
                stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"
            >
                <circle cx="8" cy="8" r="5.8" />
                <path d="M8 2.2a5.8 5.8 0 0 0 0 11.6Z" fill="currentColor" stroke="none" />
            </svg>
        },
    }
}

/// Les six pastilles rangées derrière un bouton, pour que l'en-tête tienne sur
/// une ligne avec le nom du site.
///
/// `<details>` plutôt qu'un bouton et un état, et ce n'est pas une
/// coquetterie : le site est pré-rendu pour rester lisible sans script, et
/// `<details>` s'ouvre tout seul. Sans lui, quelqu'un dont le wasm ne charge
/// pas se retrouverait sans aucun moyen de changer de langue — et le wasm pèse
/// plus lourd que le bundle qu'il remplace, donc ce cas est plus probable
/// qu'avant, pas moins. Le navigateur l'annonce aussi correctement aux
/// lecteurs d'écran, sans qu'on ait à tenir `aria-expanded` à la main — un
/// attribut qu'on oublie de mettre à jour est pire que pas d'attribut du tout.
///
/// Le script ne fait qu'ajouter le confort : refermer à l'Échap et au clic
/// au-dehors. Sans lui, on referme en retouchant le bouton, ce qui marche.
#[function_component]
pub fn HeaderMenu(props: &PageProps) -> Html {
    let texte = use_texte();
    let boite = use_node_ref();

    {
        let boite = boite.clone();
        use_effect_with((), move |_| {
            #[cfg(target_arch = "wasm32")]
            let menage = ecouter_les_fermetures(boite);
            #[cfg(not(target_arch = "wasm32"))]
            let menage = {
                let _ = boite;
                || ()
            };
            menage
        });
    }

    html! {
        <details class="menu" ref={boite}>
            <summary class="menu__bouton" aria-label={texte.copy.nav.menu_label.clone()}>
                <svg
                    viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.7"
                    stroke-linecap="round" aria-hidden="true" focusable="false"
                >
                    <line x1="3" y1="6" x2="17" y2="6" />
                    <line x1="3" y1="10" x2="17" y2="10" />
                    <line x1="3" y1="14" x2="17" y2="14" />
                </svg>
            </summary>

            // Les deux groupes portent un intitulé visible. Six pastilles en
            // rang dans l'en-tête ne disaient pas lesquelles étaient des
            // langues et lesquelles des thèmes ; ici, la place existe pour le
            // dire.
            <div class="menu__panneau">
                <p class="menu__titre">{ texte.copy.nav.language_label.clone() }</p>
                <LanguageSwitcher page={props.page} />
                <p class="menu__titre">{ texte.copy.nav.theme_label.clone() }</p>
                <ThemeSwitcher />
            </div>
        </details>
    }
}

/// Garde `path_for` sous la main des composants qui construisent une adresse
/// sans passer par le routeur.
pub fn adresse(language: Language, page: Page) -> String {
    path_for(language, page)
}

/// Referme le menu à l'Échap et au clic au-dehors.
///
/// Rend de quoi se désabonner : les deux écoutes vivent sur le document, donc
/// rien ne les retirerait si le composant disparaissait.
#[cfg(target_arch = "wasm32")]
fn ecouter_les_fermetures(boite: NodeRef) -> impl FnOnce() {
    use wasm_bindgen::JsCast;

    let fermer = {
        let boite = boite.clone();
        move || {
            if let Some(details) = boite.cast::<web_sys::HtmlDetailsElement>() {
                details.set_open(false);
            }
        }
    };

    let mut ecoutes: Vec<gloo_events::EventListener> = Vec::new();
    if let Some(document) = web_sys::window().and_then(|f| f.document()) {
        // `pointerdown` et non `click` : un menu qui se referme au
        // relâchement laisse le doigt sur ce qu'il y avait dessous.
        let au_clic = {
            let boite = boite.clone();
            let fermer = fermer.clone();
            move |evenement: &web_sys::Event| {
                let Some(details) = boite.cast::<web_sys::HtmlDetailsElement>() else {
                    return;
                };
                if !details.open() {
                    return;
                }
                let dehors = evenement
                    .target()
                    .and_then(|cible| cible.dyn_into::<web_sys::Node>().ok())
                    .map(|noeud| !details.contains(Some(&noeud)))
                    .unwrap_or(true);
                if dehors {
                    fermer();
                }
            }
        };
        ecoutes.push(gloo_events::EventListener::new(
            &document,
            "pointerdown",
            au_clic,
        ));

        let au_clavier = move |evenement: &web_sys::Event| {
            let touche = evenement.dyn_ref::<web_sys::KeyboardEvent>().map(|e| e.key());
            if touche.as_deref() == Some("Escape") {
                fermer();
            }
        };
        ecoutes.push(gloo_events::EventListener::new(
            &document,
            "keydown",
            au_clavier,
        ));
    }

    move || drop(ecoutes)
}
