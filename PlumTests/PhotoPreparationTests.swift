import XCTest
import UIKit
@testable import Plum

final class PhotoPreparationTests: XCTestCase {
    /// Une image d'une taille donnée, en JPEG.
    private func image(_ width: CGFloat, _ height: CGFloat) -> UIImage {
        let format = UIGraphicsImageRendererFormat.default()
        format.scale = 1
        return UIGraphicsImageRenderer(size: CGSize(width: width, height: height), format: format)
            .image { context in
                UIColor.systemPink.setFill()
                context.fill(CGRect(x: 0, y: 0, width: width, height: height))
            }
    }

    /// Le point n'est pas le pixel : sans `format.scale = 1`, le rendu suit
    /// l'échelle de l'écran et produit une image trois fois trop grande.
    func testResizingRespectsTheLongEdgeInPixels() {
        let reduced = PhotoPreparation.resized(image(4032, 3024), longEdge: 1600)
        XCTAssertEqual(reduced.size.width, 1600)
        XCTAssertEqual(reduced.size.height, 1200)
        XCTAssertEqual(reduced.scale, 1)
    }

    func testASmallImageIsNotBlownUp() {
        let untouched = PhotoPreparation.resized(image(600, 400), longEdge: 1600)
        XCTAssertEqual(untouched.size, CGSize(width: 600, height: 400))
    }

    /// Ce que le serveur attend : du JPEG, pas le HEIC que le sélecteur rend.
    func testThePreparedDataIsJPEG() throws {
        let source = try XCTUnwrap(image(2000, 1500).pngData())
        let prepared = try XCTUnwrap(PhotoPreparation.jpeg(from: source))

        // Les deux premiers octets d'un JPEG, quel que soit l'encodeur.
        XCTAssertEqual(Array(prepared.prefix(2)), [0xFF, 0xD8])
        let back = try XCTUnwrap(UIImage(data: prepared))
        XCTAssertEqual(back.size.width, PhotoPreparation.longEdge)
        XCTAssertLessThan(prepared.count, source.count, "l'envoi doit être plus léger que l'original")
    }

    /// Un fichier que le système ne sait pas décoder n'a aucune chance côté
    /// serveur non plus : autant s'en apercevoir avant de le téléverser.
    func testSomethingThatIsNotAnImageGivesNothing() {
        XCTAssertNil(PhotoPreparation.jpeg(from: Data("pas une image".utf8)))
    }
}
