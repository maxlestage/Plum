import SwiftUI

/// Les trois couleurs de marque, pour le code partagé avec les extensions.
///
/// L'extension de widget est une cible séparée : elle ne voit pas
/// `PlumTheme`, ni son initialiseur `Color(hex:)`. Plutôt que de recopier cet
/// initialiseur ici — deux extensions du même type dans la même application
/// entreraient en conflit — les valeurs sont écrites en composantes.
///
/// Elles ne peuvent pas dériver en silence : `Scripts/check_palette.py`
/// vérifie qu'elles valent exactement les hexadécimaux de `PlumTheme`, comme
/// il le fait déjà pour le catalogue d'actifs, le générateur d'icône et le
/// CSS du site.
public enum PlumBrand {
    /// Le nom tel qu'il s'affiche : sur l'écran d'accueil de l'iPhone, dans la
    /// galerie de widgets, sur la montre, et en haut de la sélection.
    ///
    /// Une seule fois, parce qu'un nom qui vit en cinq endroits se renomme en
    /// quatre. Le cinquième est le réglage `CFBundleDisplayName` du projet
    /// Xcode, qui n'est pas du Swift et ne peut donc pas lire cette constante :
    /// `Scripts/check_app_name.py` vérifie qu'il dit la même chose, sans quoi
    /// l'icône et l'en-tête finiraient par ne plus porter le même nom.
    ///
    /// Le nom du produit, lui, ne bouge pas : le paquet reste `Plum.app`, le
    /// module reste `Plum`, l'identifiant reste le même. Renommer l'affichage
    /// ne renomme pas l'application pour le système.
    public static let displayName = "Plum \u{2023}"

    /// `0x6B2D5C`
    public static let plum = Color(red: 0x6B / 255, green: 0x2D / 255, blue: 0x5C / 255)

    /// `0x40183A`
    public static let plumDeep = Color(red: 0x40 / 255, green: 0x18 / 255, blue: 0x3A / 255)

    /// `0xFFF7F4`
    public static let cream = Color(red: 0xFF / 255, green: 0xF7 / 255, blue: 0xF4 / 255)

    /// `0xE8608C`
    public static let blush = Color(red: 0xE8 / 255, green: 0x60 / 255, blue: 0x8C / 255)
}
