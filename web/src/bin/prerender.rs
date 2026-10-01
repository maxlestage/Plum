//! Écrit une coquille HTML statique pour chaque langue et chaque page.
//!
//! Le site est une application sur une seule page : `index.html` ne porte
//! qu'une langue, qu'un titre et qu'une description. L'application les corrige
//! une fois démarrée — ce qui suffit à une personne et ne sert à rien aux
//! robots qui fabriquent les aperçus de lien : partager `/es` montrerait le
//! titre français dans WhatsApp, iMessage ou Slack, parce qu'aucun des trois
//! n'exécute de code.
//!
//! Chaque page reçoit donc sa coquille, avec les bonnes métadonnées cuites
//! dedans **et son corps déjà rendu**. Le wasm est le même partout ; seules la
//! tête et le corps changent.
//!
//! Ce binaire remplace `scripts/prerender.mjs` à l'identique : mêmes
//! substitutions, mêmes garde-fous, même témoin vérifié à la fin. Il rend le
//! corps avec le *même* arbre de composants que le navigateur hydratera,
//! puisque c'est le même crate — ce que deux implémentations séparées, une en
//! TypeScript et une en Rust, n'auraient pas garanti.

use std::fs;
use std::path::{Path, PathBuf};

use plum_site::app::{AppStatique, StatiqueProps};
use plum_site::i18n::routes::{path_for, site, Page, PAGES};
use plum_site::i18n::{Language, LANGUAGES};

/// `&` d'abord, sinon on double-échapperait les entités ajoutées ensuite.
fn echapper(valeur: &str) -> String {
    valeur
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Remplace la première occurrence d'un motif délimité par deux bornes.
///
/// Les balises sont écrites sur plusieurs lignes par le formateur, donc on ne
/// peut pas les chercher d'un seul tenant. Plutôt qu'une expression
/// rationnelle et une dépendance de plus, on cherche le début puis la
/// première fermeture qui suit — ce que faisait la version non gourmande du
/// script d'origine.
fn remplacer_balise(page: &str, debut: &str, remplacement: &str) -> String {
    let Some(ouverture) = page.find(debut) else {
        panic!("le gabarit ne porte plus « {debut} »");
    };
    let reste = &page[ouverture..];
    let Some(fermeture) = reste.find("/>") else {
        panic!("la balise « {debut} » ne se referme pas");
    };
    let mut sortie = String::with_capacity(page.len() + remplacement.len());
    sortie.push_str(&page[..ouverture]);
    sortie.push_str(remplacement);
    sortie.push_str(&reste[fermeture + 2..]);
    sortie
}

fn remplacer_une_fois(page: &str, motif: &str, remplacement: &str) -> String {
    assert!(page.contains(motif), "le gabarit ne porte plus « {motif} »");
    page.replacen(motif, remplacement, 1)
}

struct Contexte {
    origine: String,
}

impl Contexte {
    /// Une adresse absolue.
    ///
    /// Google ignore un `hreflang` relatif, et recommande un `canonical`
    /// absolu. Les coquilles sont précisément ce que les robots lisent —
    /// l'application, à l'exécution, les réécrit en absolu à partir de
    /// l'origine réellement servie, donc l'erreur ne se verrait que là où
    /// elle compte.
    fn absolu(&self, chemin: &str) -> String {
        format!("{}{}", self.origine, chemin)
    }

    /// Les balises de l'aperçu de lien.
    ///
    /// Sans elles, un lien envoyé sur WhatsApp, iMessage ou Slack s'affiche en
    /// texte nu. `og:image` doit être absolue — un aperçu est construit par un
    /// serveur qui n'a aucune page de référence pour résoudre un chemin — d'où
    /// leur place ici plutôt que dans le gabarit, où l'adresse du déploiement
    /// devrait être recopiée.
    ///
    /// `summary_large_image` demande la grande vignette plutôt que la
    /// miniature carrée, qui rognerait la marque.
    fn apercu(&self, language: Language, alt: &str) -> String {
        let _ = language;
        [
            format!(
                "<meta property=\"og:image\" content=\"{}\" />",
                self.absolu("/partage.png")
            ),
            "<meta property=\"og:image:width\" content=\"1200\" />".to_string(),
            "<meta property=\"og:image:height\" content=\"630\" />".to_string(),
            format!(
                "<meta property=\"og:image:alt\" content=\"{}\" />",
                echapper(alt)
            ),
            "<meta name=\"twitter:card\" content=\"summary_large_image\" />".to_string(),
        ]
        .join("\n    ")
    }

    /// Les annotations trilingues d'une page, `x-default` compris.
    fn alternates(&self, page: Page) -> String {
        let mut lignes = Vec::new();
        for language in LANGUAGES {
            lignes.push(format!(
                "<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\" />",
                language.code(),
                self.absolu(&path_for(language, page))
            ));
        }
        lignes.push(format!(
            "<link rel=\"alternate\" hreflang=\"x-default\" href=\"{}\" />",
            self.absolu(&path_for(Language::Fr, page))
        ));
        lignes.join("\n    ")
    }
}

fn rendre(chemin: &str) -> String {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("une boucle sur ce fil");
    let chemin = chemin.to_string();
    runtime.block_on(async move {
        yew::ServerRenderer::<AppStatique>::with_props(move || StatiqueProps { chemin })
            .render()
            .await
    })
}

/// Recopie les actifs de `public/` à la racine de `dist/`.
///
/// Trunk sait copier un dossier, mais en dossier : `public/favicon.svg`
/// atterrirait sur `/public/favicon.svg`, alors que le gabarit et le manifeste
/// le demandent à la racine. Dix fichiers, une boucle, et le chemin des icônes
/// reste celui qu'on lit dans `index.html`.
fn recopier_les_actifs(source: &Path, dist: &Path) {
    let Ok(entrees) = fs::read_dir(source) else {
        panic!("{} est introuvable", source.display());
    };
    for entree in entrees.flatten() {
        let chemin = entree.path();
        if chemin.is_file() {
            let destination = dist.join(chemin.file_name().expect("un nom de fichier"));
            fs::copy(&chemin, &destination)
                .unwrap_or_else(|e| panic!("{} : {e}", destination.display()));
        }
    }
}

fn main() {
    let racine = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dist = racine.join("dist");
    let site = site();
    let contexte = Contexte {
        origine: site.origin.clone(),
    };

    let gabarit = fs::read_to_string(dist.join("index.html"))
        .expect("dist/index.html : lancez `trunk build` avant le pré-rendu");

    // Le gabarit est aussi une des sorties : la racine reçoit ses annotations
    // comme les douze autres pages. Relancé sans `trunk build`, ce binaire
    // lirait donc sa propre production et injecterait tout une seconde fois —
    // quatre balises en double, qu'aucun aperçu ne sait départager.
    assert!(
        !gabarit.contains("rel=\"canonical\""),
        "dist/index.html porte déjà ses annotations — relancez `trunk build`,\n\
         ce binaire doit partir d'une construction fraîche."
    );

    recopier_les_actifs(&racine.join("public"), &dist);

    let mut ecrites = 0usize;

    for language in LANGUAGES {
        let meta = site
            .meta
            .get(language.code())
            .expect("site.json décrit les trois langues");

        for page in PAGES {
            let titre = match meta.page_titles.get(page.key()).and_then(Option::as_deref) {
                Some(section) => format!("{section} — Plum"),
                None => meta.title.clone(),
            };

            let mut html = remplacer_une_fois(
                &gabarit,
                "<html lang=\"fr\">",
                &format!("<html lang=\"{}\">", language.code()),
            );
            // `remplacer_balise` cherche `/>` : le titre se referme par
            // `</title>`, donc il se traite à part.
            let debut_titre = html.find("<title>").expect("le gabarit porte un titre");
            let fin_titre = html.find("</title>").expect("le titre se referme") + "</title>".len();
            html = format!(
                "{}<title>{}</title>{}",
                &html[..debut_titre],
                echapper(&titre),
                &html[fin_titre..]
            );

            html = remplacer_balise(
                &html,
                "<meta\n      name=\"description\"",
                &format!(
                    "<meta name=\"description\" content=\"{}\" />",
                    echapper(&meta.description)
                ),
            );
            html = remplacer_balise(
                &html,
                "<meta property=\"og:title\"",
                &format!(
                    "<meta property=\"og:title\" content=\"{}\" />",
                    echapper(&titre)
                ),
            );
            html = remplacer_balise(
                &html,
                "<meta\n      property=\"og:description\"",
                &format!(
                    "<meta property=\"og:description\" content=\"{}\" />\n    \
                     <meta property=\"og:locale\" content=\"{}\" />\n    {}\n    \
                     <link rel=\"canonical\" href=\"{}\" />\n    {}",
                    echapper(&meta.description),
                    language.code(),
                    contexte.apercu(language, &meta.image_alt),
                    contexte.absolu(&path_for(language, page)),
                    contexte.alternates(page),
                ),
            );

            // Le corps, rendu ici plutôt que laissé au navigateur. Sans lui,
            // un robot qui n'exécute rien voit le bon titre et rien dessous.
            let corps = rendre(&path_for(language, page));
            assert!(
                corps.contains("</"),
                "{} n'a rien rendu — l'arbre a dû changer",
                path_for(language, page)
            );

            let html = remplacer_une_fois(
                &html,
                "<div id=\"root\"></div>",
                &format!("<div id=\"root\">{corps}</div>"),
            );

            let dossier = dist.join(path_for(language, page).trim_start_matches('/'));
            fs::create_dir_all(&dossier).expect("le dossier de la page");
            fs::write(dossier.join("index.html"), html).expect("la page");
            ecrites += 1;
        }
    }

    // La racine, qui est l'adresse qu'on partage.
    //
    // `/` n'est pas une page : l'application y choisit une langue et renvoie
    // vers `/fr/`. Mais c'est une redirection côté client, donc un robot qui
    // n'exécute rien s'arrête là — et il y trouvait le gabarit brut, sans
    // `canonical` ni `hreflang`, précisément sur l'adresse la plus liée du
    // site. Elle porte donc les annotations de la version française, vers
    // laquelle elle mène.
    {
        let meta = site.meta.get("fr").expect("site.json décrit le français");
        let racine_html = remplacer_balise(
            &gabarit,
            "<meta\n      property=\"og:description\"",
            &format!(
                "<meta property=\"og:description\" content=\"{}\" />\n    \
                 <meta property=\"og:locale\" content=\"fr\" />\n    {}\n    \
                 <link rel=\"canonical\" href=\"{}\" />\n    {}",
                echapper(&meta.description),
                contexte.apercu(Language::Fr, &meta.image_alt),
                contexte.absolu(&path_for(Language::Fr, Page::Home)),
                contexte.alternates(Page::Home),
            ),
        );
        assert!(
            racine_html.contains("rel=\"canonical\""),
            "la racine n'a pas reçu son canonical"
        );
        assert!(
            racine_html.contains(&format!("<title>{}</title>", echapper(&meta.title))),
            "le gabarit ne porte plus le titre français"
        );
        fs::write(dist.join("index.html"), racine_html).expect("la racine");
        ecrites += 1;
    }

    // Une coquille qui n'aurait fait aucune de ses substitutions partirait en
    // silence : correcte pour une personne, fausse pour chaque robot.
    let temoin = fs::read_to_string(dist.join("es/privacidad/index.html")).expect("le témoin");
    let attendus = [
        "<html lang=\"es\">".to_string(),
        "Privacidad — Plum".to_string(),
        "hreflang=\"x-default\"".to_string(),
        format!("rel=\"canonical\" href=\"{}/es/privacidad/\"", site.origin),
        format!("hreflang=\"fr\" href=\"{}/fr/confidentialite/\"", site.origin),
        format!("og:image\" content=\"{}/partage.png\"", site.origin),
        "name=\"twitter:card\" content=\"summary_large_image\"".to_string(),
        // Le corps rendu : sans cette ligne, une régression du pré-rendu
        // repasserait en silence et ne se verrait qu'au référencement.
        "Privacidad".to_string(),
    ];
    for attendu in attendus {
        assert!(
            temoin.contains(&attendu),
            "« {attendu} » absent du témoin, le gabarit a dû changer"
        );
    }

    println!("prerender : {ecrites} pages écrites");
}
