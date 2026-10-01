//! Le point d'entrée du navigateur.
//!
//! Les douze pages traduites arrivent déjà rendues : on les hydrate, ce qui
//! évite de reconstruire un DOM identique et laisse le texte lisible avant
//! même que le wasm ne démarre.
//!
//! `with_root` et non `new` : `new` hydrate `<body>`, alors que le corps
//! pré-rendu vit dans `#root`. La différence ne se voit pas à la compilation
//! et ne se voit pas non plus à l'écran — la page s'affiche, elle est juste
//! morte. Les huit contrôles de thème l'ont attrapée d'un coup : toutes les
//! vérifications qui cliquaient une pastille échouaient, toutes celles qui
//! lisaient l'état par défaut passaient.

/// Hydrater ou rendre, selon ce qu'il y a dans le conteneur.
///
/// Les douze pages traduites sont pré-rendues : leur `#root` contient un DOM
/// et les marqueurs d'hydratation de Yew, donc on hydrate. **La racine `/`
/// n'est pas pré-rendue** : l'application y choisit une langue d'après le
/// navigateur avant de rendre quoi que ce soit, donc son `#root` est vide, et
/// c'est voulu — il n'y a pas de contenu neutre à écrire pour trois langues.
///
/// Hydrater ce vide échouait. Mesuré sur la production : Yew panique dans
/// `fragment.rs` sur « expected Component opening tag, found EOF », puis le
/// wasm meurt sur un `unreachable`. L'adresse nue du site rendait une page
/// blanche — celle qu'on obtient en tapant le domaine sans rien derrière,
/// c'est-à-dire la première que quelqu'un essaie.
///
/// Pourquoi ça n'a pas été vu : les deux gardes du navigateur visitaient `/`
/// — mais l'un n'y mesurait que le débordement, vrai sur une page blanche, et
/// l'autre ne chargeait que `/fr/`. Et l'oracle qui a validé le portage
/// comparait le texte visible : il était vide avant comme après, donc
/// identique. Trois contrôles verts sur une page morte.
/// `verifier-demarrage.mjs` est le garde qui manquait.
fn main() {
    let racine = web_sys::window()
        .and_then(|fenetre| fenetre.document())
        .and_then(|document| document.get_element_by_id("root"))
        .expect("#root : le gabarit porte ce conteneur, et le pré-rendu écrit dedans");

    let pre_rendu = racine.has_child_nodes();
    let moteur = yew::Renderer::<plum_site::app::App>::with_root(racine);
    if pre_rendu {
        moteur.hydrate();
    } else {
        moteur.render();
    }
}
