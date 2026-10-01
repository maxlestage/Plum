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
fn main() {
    let racine = web_sys::window()
        .and_then(|fenetre| fenetre.document())
        .and_then(|document| document.get_element_by_id("root"))
        .expect("#root : le gabarit porte ce conteneur, et le pré-rendu écrit dedans");

    yew::Renderer::<plum_site::app::App>::with_root(racine).hydrate();
}
