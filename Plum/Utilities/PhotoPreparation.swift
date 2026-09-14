import Foundation
import UIKit

/// Ce qu'une photo devient avant de quitter le téléphone.
///
/// Le sélecteur rend le fichier d'origine, et sur un iPhone c'est du **HEIC**
/// de plusieurs mégaoctets — un format que le serveur ne sait pas lire et
/// refuse, à raison plutôt que d'embarquer une bibliothèque C pour lui. La
/// conversion se fait donc ici, où l'appareil a déjà le décodeur dans le
/// système.
///
/// Réduire avant d'envoyer sert aussi la personne en face : téléverser quatre
/// mégaoctets sur un réseau de métro prend le temps qu'il faut pour abandonner.
/// Le serveur réduit de toute façon ce qu'il reçoit — c'est lui qui décide,
/// puisqu'il ne peut pas croire le client — mais il n'a aucune raison de
/// recevoir dix fois ce qu'il gardera.
enum PhotoPreparation {
    /// Le plus grand côté envoyé. Le serveur range à 1200 ; envoyer un peu
    /// plus lui laisse de quoi travailler sans transporter l'inutile.
    static let longEdge: CGFloat = 1600

    /// 0.85 avant l'envoi, le serveur réencodant ensuite à 0.80. Compresser
    /// deux fois dégrade, donc la première passe reste généreuse.
    static let quality: CGFloat = 0.85

    /// Décode, réduit, réencode en JPEG. `nil` si le fichier n'est pas une
    /// image que le système sait lire — auquel cas le serveur n'aurait rien pu
    /// en faire non plus.
    static func jpeg(from data: Data) -> Data? {
        guard let image = UIImage(data: data) else { return nil }
        return resized(image, longEdge: longEdge).jpegData(compressionQuality: quality)
    }

    /// Réduit en gardant les proportions. Une image déjà plus petite est
    /// rendue telle quelle : l'agrandir n'inventerait aucun détail.
    static func resized(_ image: UIImage, longEdge: CGFloat) -> UIImage {
        let size = image.size
        let longest = max(size.width, size.height)
        guard longest > longEdge, longest > 0 else { return image }

        let scale = longEdge / longest
        let target = CGSize(width: (size.width * scale).rounded(),
                            height: (size.height * scale).rounded())

        // `format.scale = 1` : sans ça le rendu applique l'échelle de l'écran
        // et produit une image trois fois plus grande que demandée — le point
        // n'est pas le pixel.
        let format = UIGraphicsImageRendererFormat.default()
        format.scale = 1
        format.opaque = true

        return UIGraphicsImageRenderer(size: target, format: format).image { _ in
            image.draw(in: CGRect(origin: .zero, size: target))
        }
    }
}
