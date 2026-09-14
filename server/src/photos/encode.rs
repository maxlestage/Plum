//! Ce que devient une photo entre le téléphone et la base.
//!
//! Rien n'est stocké tel qu'il arrive. Le décodage sert de contrôle — un
//! fichier qui n'est pas une image échoue ici plutôt que d'être rangé puis
//! servi — et le réencodage fait trois choses d'un coup : il borne la taille,
//! il uniformise le format, et il **efface les métadonnées**.
//!
//! Ce dernier point n'est pas un détail de propreté. Une photo sortie d'un
//! téléphone porte ses coordonnées GPS dans son EXIF ; publiée telle quelle
//! sur une application de rencontres, elle donne l'adresse de qui la publie à
//! quiconque sait ouvrir un fichier. Réencoder la supprime, et c'est la raison
//! principale de le faire.

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader};
use std::io::Cursor;

/// Le plus grand côté d'une photo stockée.
///
/// Une carte de profil occupe la largeur d'un téléphone, soit environ 1200
/// pixels sur un écran moderne. Au-delà on stocke des pixels que personne ne
/// verra, et chacun se paie deux fois : en place et en temps de chargement.
pub const LONG_EDGE: u32 = 1200;

/// Les qualités JPEG essayées, dans l'ordre.
///
/// 80 est le point où l'on cesse de voir la différence et où le poids cesse de
/// baisser vite — mais le poids d'un JPEG dépend d'abord de ce qu'il y a
/// dedans, pas du réglage. Une photo douce tombe à 60 Kio à ce réglage ; un
/// motif à arêtes vives en fait 400, mesuré contre la production. Sans
/// deuxième passe, le plafond de stockage annoncé ne vaudrait que pour les
/// images faciles.
const QUALITIES: [u8; 5] = [80, 66, 52, 40, 30];

/// Le poids visé. Au-delà, on retente moins fin.
///
/// C'est ce chiffre qui rend vraie la phrase « de l'ordre du millier de
/// profils » : six photos par profil, un gigaoctet de plan. Le changer sans
/// changer l'autre rendrait la documentation fausse, et le test de poids le
/// dira.
const BUDGET: usize = 200 * 1024;

/// Ce qu'on accepte de recevoir avant même de décoder.
///
/// Généreux à dessein : une photo d'iPhone non réduite pèse quelques
/// mégaoctets, et refuser à quatre reviendrait à refuser des gens plutôt que
/// des fichiers. Ce qui compte est la taille *après* réencodage.
pub const MAX_UPLOAD: usize = 12 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    #[error("Cette image est trop lourde.")]
    TooLarge,
    #[error("Ce fichier n'est pas une image que nous savons lire. Le JPEG et le PNG fonctionnent ; le HEIC de l'iPhone doit être converti avant l'envoi.")]
    Unreadable,
    #[error("Cette image est trop petite pour un profil.")]
    TooSmall,
}

pub struct Encoded {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub content_type: &'static str,
}

/// En dessous, ce n'est pas une photo de profil mais une vignette : elle
/// s'afficherait floue sur toute la largeur d'un écran.
const MIN_EDGE: u32 = 200;

/// Décode, réduit, réencode. Le résultat est un JPEG sans métadonnées.
pub fn prepare(raw: &[u8]) -> Result<Encoded, PhotoError> {
    if raw.len() > MAX_UPLOAD {
        return Err(PhotoError::TooLarge);
    }

    // Le format est deviné sur le contenu, jamais sur le nom du fichier : une
    // extension est une affirmation du client, pas une preuve.
    let reader = ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|_| PhotoError::Unreadable)?;

    // Les dimensions sont lues avant le décodage : une image de 60 000 sur
    // 60 000 pixels tient dans quelques kilo-octets compressés et réclame dix
    // gigaoctets une fois décompressée. Le décodeur les impose donc avant
    // d'allouer quoi que ce soit.
    let mut decoder = reader.into_decoder().map_err(|_| PhotoError::Unreadable)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    image::ImageDecoder::set_limits(&mut decoder, limits).map_err(|_| PhotoError::TooLarge)?;

    let decoded = DynamicImage::from_decoder(decoder).map_err(|_| PhotoError::Unreadable)?;

    if decoded.width() < MIN_EDGE || decoded.height() < MIN_EDGE {
        return Err(PhotoError::TooSmall);
    }

    // `thumbnail` irait plus vite et rendrait des escaliers sur les bords ;
    // c'est la première chose qu'on voit d'une personne.
    let resized = if decoded.width().max(decoded.height()) > LONG_EDGE {
        decoded.resize(LONG_EDGE, LONG_EDGE, FilterType::Lanczos3)
    } else {
        decoded
    };

    // La transparence n'a pas de sens sur une photo de profil, et le JPEG ne
    // la garde pas : convertie explicitement, sinon les zones transparentes
    // d'un PNG ressortent en noir.
    let flattened = DynamicImage::ImageRgb8(resized.to_rgb8());

    // La première qualité qui tient dans le budget, sinon la dernière. Deux
    // réencodages de plus dans le pire cas, sur une image déjà réduite : le
    // coût se compte en millisecondes, et il est payé une fois pour une photo
    // qui sera servie des milliers de fois.
    let mut bytes = Vec::new();
    for (index, quality) in QUALITIES.iter().enumerate() {
        bytes.clear();
        flattened
            .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, *quality))
            .map_err(|_| PhotoError::Unreadable)?;
        if bytes.len() <= BUDGET || index == QUALITIES.len() - 1 {
            break;
        }
    }

    Ok(Encoded {
        width: flattened.width(),
        height: flattened.height(),
        bytes,
        content_type: "image/jpeg",
    })
}

/// Ce qu'on sait dire de ce qu'on a produit, pour l'en-tête `Content-Type`.
pub fn format_of(content_type: &str) -> ImageFormat {
    match content_type {
        "image/png" => ImageFormat::Png,
        _ => ImageFormat::Jpeg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let mut canvas = RgbImage::new(width, height);
        // Un dégradé plutôt qu'un aplat : un aplat se comprime à presque rien
        // et ne dirait rien des tailles réelles.
        for (x, y, pixel) in canvas.enumerate_pixels_mut() {
            *pixel = Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]);
        }
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(canvas)
            .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, 95))
            .unwrap();
        bytes
    }

    #[test]
    fn a_large_photo_comes_back_within_the_long_edge() {
        let prepared = prepare(&jpeg(4032, 3024)).expect("une photo d'iPhone");
        assert_eq!(prepared.width, LONG_EDGE);
        assert_eq!(prepared.height, 900, "les proportions sont gardées");
        assert_eq!(prepared.content_type, "image/jpeg");
    }

    /// Agrandir une petite photo ne créerait pas de détail, seulement du
    /// poids et du flou.
    #[test]
    fn a_small_photo_is_not_blown_up() {
        let prepared = prepare(&jpeg(600, 800)).expect("une petite photo");
        assert_eq!((prepared.width, prepared.height), (600, 800));
    }

    #[test]
    fn a_thumbnail_is_refused_rather_than_stretched() {
        assert!(matches!(
            prepare(&jpeg(120, 120)),
            Err(PhotoError::TooSmall)
        ));
    }

    /// Le contrôle de format est le décodage lui-même : rien d'autre n'est
    /// nécessaire pour écarter ce qui n'est pas une image.
    #[test]
    fn anything_that_is_not_an_image_is_refused() {
        for raw in [
            &b"MZ\x90\x00ceci est un ex\xC3\xA9cutable"[..],
            b"<svg xmlns='http://www.w3.org/2000/svg'></svg>",
            b"",
        ] {
            assert!(
                matches!(prepare(raw), Err(PhotoError::Unreadable)),
                "{raw:?}"
            );
        }
    }

    #[test]
    fn an_oversized_upload_is_refused_before_decoding() {
        assert!(matches!(
            prepare(&vec![0u8; MAX_UPLOAD + 1]),
            Err(PhotoError::TooLarge)
        ));
    }

    /// Un PNG transparent réencodé en JPEG doit rendre du blanc plutôt que du
    /// noir : `to_rgb8` écrase l'alpha, et sans conversion explicite les zones
    /// transparentes viraient au noir.
    #[test]
    fn a_transparent_png_does_not_turn_black() {
        use image::{ImageBuffer, Rgba};
        let canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(400, 400, Rgba([255, 255, 255, 0]));
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(canvas)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();

        let prepared = prepare(&png).expect("un PNG");
        let back = image::load_from_memory(&prepared.bytes).unwrap().to_rgb8();
        let Rgb([r, g, b]) = *back.get_pixel(200, 200);
        assert!(r > 200 && g > 200 && b > 200, "obtenu {r},{g},{b}");
    }
}

#[cfg(test)]
mod poids {
    use super::*;
    use image::{Rgb, RgbImage};

    /// Une image qui se comprime comme une photographie.
    ///
    /// Ni un aplat — qui tomberait à quelques kilo-octets et flatterait le
    /// résultat — ni du bruit par pixel, qui est le pire cas absolu du JPEG et
    /// pesait ici dix-sept mégaoctets avant même d'entrer. Une photographie
    /// est entre les deux : de larges zones douces, des détails à plusieurs
    /// échelles, un peu de grain.
    fn photograph(width: u32, height: u32) -> Vec<u8> {
        let mut canvas = RgbImage::new(width, height);
        let mut seed: u32 = 0x2545_f491;
        for (x, y, pixel) in canvas.enumerate_pixels_mut() {
            let (fx, fy) = (x as f32 / width as f32, y as f32 / height as f32);
            // Trois échelles : le sujet, sa texture, son grain.
            let broad = (fx * 6.0).sin() * (fy * 4.0).cos();
            let detail = (fx * 60.0).sin() * (fy * 47.0).sin() * 0.25;
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let grain = ((seed >> 24) as f32 / 255.0 - 0.5) * 0.06;
            let value = ((broad + detail + grain) * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0;
            let v = value as u8;
            *pixel = Rgb([v, v.saturating_sub(18), v.saturating_add(12)]);
        }
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(canvas)
            .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, 92))
            .unwrap();
        bytes
    }

    /// Ce que pèse réellement une photo une fois rangée.
    ///
    /// Le plafond annoncé dans la migration repose sur ce chiffre. Il est
    /// mesuré ici plutôt qu'estimé, et ce test échouera le jour où un réglage
    /// le fera dériver, au lieu de laisser le commentaire mentir en silence.
    #[test]
    fn a_stored_photo_weighs_what_the_ceiling_assumes() {
        let raw = photograph(4032, 3024);
        let prepared = prepare(&raw).expect("une photo");
        let kio = prepared.bytes.len() / 1024;
        println!(
            "reçue {} Kio → stockée {kio} Kio ({}×{})",
            raw.len() / 1024,
            prepared.width,
            prepared.height
        );

        assert!(
            kio <= BUDGET / 1024,
            "une photo pèse {kio} Kio : le plafond annoncé dans la migration \
             ne tient plus, corrigez l'un ou l'autre"
        );
    }

    /// Le cas qui a fait ajouter la deuxième passe : un motif à arêtes vives
    /// pesait 401 Kio, mesuré contre la production, alors que la documentation
    /// promettait de l'ordre de 150.
    #[test]
    fn even_the_worst_case_stays_within_the_budget() {
        let mut canvas = RgbImage::new(1800, 1350);
        for (x, y, pixel) in canvas.enumerate_pixels_mut() {
            // Une dent de scie à haute fréquence : ce que le JPEG comprime le
            // plus mal sans être du bruit pur.
            *pixel = Rgb([
                ((x * 7) % 256) as u8,
                ((y * 5) % 256) as u8,
                ((x + y) % 256) as u8,
            ]);
        }
        let mut raw = Vec::new();
        DynamicImage::ImageRgb8(canvas)
            .write_to(&mut Cursor::new(&mut raw), ImageFormat::Png)
            .unwrap();

        let prepared = prepare(&raw).expect("une image difficile");
        let kio = prepared.bytes.len() / 1024;
        println!("pire cas : {kio} Kio");
        assert!(kio <= BUDGET / 1024, "le pire cas pèse {kio} Kio");
    }
}
