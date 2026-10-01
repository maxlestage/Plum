//! La marque : deux disques qui se chevauchent, et au milieu une troisième
//! forme que ni l'un ni l'autre n'a dessinée.
//!
//! Chaque moitié est un disque dont on retire un second disque décalé — d'où
//! le `fill-rule="evenodd"` : ce qui appartient à l'un *ou* à l'autre, jamais
//! aux deux. C'est ce qui donne l'épaisseur variable d'un trait de plume,
//! épais sur les flancs et affiné en haut et en bas.
//!
//! La même géométrie, aux mêmes coordonnées dans le même repère de 64 unités,
//! est écrite dans `public/favicon.svg` et dans `Scripts/generate_appicon.py`.
//! Trois rendus d'un seul dessin : si l'un change, les deux autres doivent
//! suivre.
//!
//! Aucun dégradé et aucun identifiant : la marque est en aplat, donc rien à
//! référencer, donc rien à faire entrer en collision quand elle paraît deux
//! fois sur la même page.

use yew::prelude::*;

/// Le tracé de gauche, puis celui de droite. Écrits ici en constantes pour
/// que le dessin se lise d'un bloc plutôt que noyé dans le `html!`.
const GAUCHE: &str = "M23.5 14.5 a17.5 17.5 0 1 0 0 35 a17.5 17.5 0 1 0 0 -35 Z \
                      M30 16.8 a15.2 15.2 0 1 1 0 30.4 a15.2 15.2 0 1 1 0 -30.4 Z";
const DROITE: &str = "M40.5 14.5 a17.5 17.5 0 1 1 0 35 a17.5 17.5 0 1 1 0 -35 Z \
                      M34 16.8 a15.2 15.2 0 1 0 0 30.4 a15.2 15.2 0 1 0 0 -30.4 Z";

#[derive(Properties, PartialEq)]
pub struct PlumMarkProps {
    #[prop_or(64)]
    pub size: u32,
}

#[function_component]
pub fn PlumMark(props: &PlumMarkProps) -> Html {
    let taille = props.size.to_string();
    html! {
        <svg
            width={taille.clone()}
            height={taille}
            viewBox="0 0 64 64"
            role="img"
            aria-label="Plum"
        >
            <circle cx="32" cy="32" r="32" fill="#40183A" />
            <path d={GAUCHE} fill="#FFF7F4" fill-rule="evenodd" />
            <path d={DROITE} fill="#FFF7F4" fill-rule="evenodd" />
        </svg>
    }
}
