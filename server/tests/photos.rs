//! Les photos, de l'envoi à l'affichage, contre un vrai Postgres.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::deck_ages::*;
use common::*;
use http_body_util::BodyExt;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::json;
use tower::ServiceExt;

/// Un JPEG de la taille demandée, avec assez de détail pour ne pas se
/// comprimer à rien.
fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let mut canvas = RgbImage::new(width, height);
    for (x, y, pixel) in canvas.enumerate_pixels_mut() {
        *pixel = Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]);
    }
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(canvas)
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Jpeg)
        .unwrap();
    bytes
}

/// Un envoi multipart, comme `URLSession` le compose.
fn upload_request(token: &str, bytes: Vec<u8>) -> Request<Body> {
    let boundary = "plum.test.boundary";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; \
             filename=\"photo.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    Request::builder()
        .method("POST")
        .uri("/api/v1/me/photos")
        .header("authorization", format!("Bearer {token}"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

/// La carte d'un candidat, en parcourant le deck page par page.
///
/// Le deck lit toute la base et la suite en partage une, donc la carte
/// cherchée n'est pas forcément sur la première page.
async fn deck_card(
    app: &axum::Router,
    token: &str,
    wanted: uuid::Uuid,
) -> Option<serde_json::Value> {
    let mut query = "?limit=50".to_owned();
    for _ in 0..40 {
        let (status, body) = call(
            app,
            request(
                "GET",
                &format!("/api/v1/discovery/deck{query}"),
                Some(token),
                None,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "deck : {body}");

        if let Some(found) = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == wanted.to_string())
        {
            return Some(found.clone());
        }

        match body["next_cursor"].as_str() {
            Some(cursor) => query = format!("?limit=50&cursor={}", cursor.replace('|', "%7C")),
            None => return None,
        }
    }
    None
}

async fn upload(app: &axum::Router, token: &str, size: (u32, u32)) -> serde_json::Value {
    let (status, body) = call(app, upload_request(token, jpeg(size.0, size.1))).await;
    assert_eq!(status, StatusCode::OK, "envoi : {body}");
    body
}

/// La forme est le contrat : le modèle Swift décode `id`, `url`, `position`,
/// et `url` doit être absolue — `AsyncImage` ne résout pas les chemins.
#[tokio::test]
async fn an_uploaded_photo_comes_back_in_the_shape_the_client_decodes() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "photo-forme").await;

    let photo = upload(&app, &token, (1600, 1200)).await;

    assert!(photo["id"].is_string());
    assert_eq!(photo["position"], 0, "la première photo est la couverture");
    let url = photo["url"].as_str().expect("une adresse");
    assert!(
        url.starts_with("https://plum.test/photos/"),
        "adresse relative ou inattendue : {url}"
    );
    assert!(url.ends_with(photo["id"].as_str().unwrap()));
}

/// Sans ça, la photo est rangée et jamais montrée.
#[tokio::test]
async fn the_bytes_come_back_from_the_public_address_without_a_token() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "photo-servie").await;
    let photo = upload(&app, &token, (1600, 1200)).await;
    let id = photo["id"].as_str().unwrap();

    // Aucun en-tête d'authentification, délibérément : c'est ainsi que
    // `AsyncImage` demande.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/photos/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/jpeg");
    assert!(
        response.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("immutable"),
        "sans cache, chaque ouverture du deck redemanderait vingt images"
    );

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let decoded = image::load_from_memory(&bytes).expect("une image lisible");
    assert_eq!(decoded.width(), 1200, "réduite au grand côté");
}

/// Le réencodage n'est pas cosmétique : une photo sortie d'un téléphone porte
/// ses coordonnées GPS, et les publier sur une application de rencontres
/// donnerait l'adresse de qui les publie.
#[tokio::test]
async fn the_stored_photo_carries_none_of_the_metadata_it_arrived_with() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "photo-exif").await;

    // Un JPEG avec un segment EXIF fabriqué, contenant une chaîne qu'on
    // pourra chercher dans la sortie.
    let mut raw = jpeg(800, 600);
    let marker = b"PLUM-GPS-48.8566-2.3522";
    let mut exif = vec![0xFF, 0xE1];
    let payload_len = (marker.len() + 8) as u16;
    exif.extend_from_slice(&payload_len.to_be_bytes());
    exif.extend_from_slice(b"Exif\0\0");
    exif.extend_from_slice(marker);
    // Juste après le SOI (deux octets), là où un appareil photo l'écrirait.
    raw.splice(2..2, exif);

    let (status, body) = call(&app, upload_request(&token, raw)).await;
    assert_eq!(status, StatusCode::OK, "envoi : {body}");

    let id = body["id"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/photos/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let stored = response.into_body().collect().await.unwrap().to_bytes();

    assert!(
        !stored.windows(marker.len()).any(|w| w == marker),
        "les métadonnées d'origine sont encore là"
    );
}

#[tokio::test]
async fn a_file_that_is_not_an_image_is_refused_with_something_to_do_about_it() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "photo-pas-image").await;

    let (status, body) = call(
        &app,
        upload_request(&token, b"MZ\x90\x00 pas une image".to_vec()),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let message = body["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("HEIC"),
        "le message doit dire quoi faire, pas seulement que ça a raté : {message}"
    );
}

/// Le client affiche six au maximum ; une limite qui n'existe que dans
/// l'application n'existe pas.
#[tokio::test]
async fn a_seventh_photo_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "photo-sept").await;

    for index in 0..6 {
        let photo = upload(&app, &token, (800, 600)).await;
        assert_eq!(photo["position"], index, "les positions se suivent");
    }

    let (status, body) = call(&app, upload_request(&token, jpeg(800, 600))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

/// Sans le resserrage, retirer la couverture laisserait la position 0 vide et
/// aucune photo ne deviendrait jamais couverture.
#[tokio::test]
async fn removing_the_cover_promotes_the_next_one() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "photo-retrait").await;

    let first = upload(&app, &token, (800, 600)).await;
    let second = upload(&app, &token, (800, 600)).await;
    let third = upload(&app, &token, (800, 600)).await;

    let (status, _) = call(
        &app,
        request(
            "DELETE",
            &format!("/api/v1/me/photos/{}", first["id"].as_str().unwrap()),
            Some(&token),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, profile) = call(
        &app,
        request("GET", "/api/v1/me/profile", Some(&token), None),
    )
    .await;
    let photos = profile["photos"].as_array().expect("des photos");
    assert_eq!(photos.len(), 2);
    assert_eq!(photos[0]["id"], second["id"]);
    assert_eq!(photos[0]["position"], 0, "la suivante devient couverture");
    assert_eq!(photos[1]["id"], third["id"]);
    assert_eq!(photos[1]["position"], 1);
}

#[tokio::test]
async fn the_photos_of_someone_else_cannot_be_removed() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (mine, _) = sign_up_and_token(&app, "photo-mienne").await;
    let (theirs, _) = sign_up_and_token(&app, "photo-autrui").await;

    let photo = upload(&app, &mine, (800, 600)).await;
    let (status, _) = call(
        &app,
        request(
            "DELETE",
            &format!("/api/v1/me/photos/{}", photo["id"].as_str().unwrap()),
            Some(&theirs),
            None,
        ),
    )
    .await;
    // Introuvable plutôt qu'interdit : confirmer l'existence renseignerait.
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, profile) = call(
        &app,
        request("GET", "/api/v1/me/profile", Some(&mine), None),
    )
    .await;
    assert_eq!(profile["photos"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn reordering_puts_the_chosen_photo_on_the_cover() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "photo-ordre").await;

    let first = upload(&app, &token, (800, 600)).await;
    let second = upload(&app, &token, (800, 600)).await;

    let (status, body) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/photos/order",
            Some(&token),
            Some(json!({ "photo_ids": [second["id"], first["id"]] })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let photos = body.as_array().expect("la liste réordonnée");
    assert_eq!(photos[0]["id"], second["id"]);
    assert_eq!(photos[0]["position"], 0);
    assert_eq!(photos[1]["id"], first["id"]);
}

/// Une liste incomplète laisserait des photos à une position arbitraire ;
/// une liste mélangée avec celle d'autrui déplacerait les photos d'un inconnu.
#[tokio::test]
async fn a_reordering_that_is_not_exactly_ones_own_photos_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (mine, _) = sign_up_and_token(&app, "photo-ordre-faux").await;
    let (theirs, _) = sign_up_and_token(&app, "photo-ordre-autrui").await;

    let one = upload(&app, &mine, (800, 600)).await;
    let two = upload(&app, &mine, (800, 600)).await;
    let alien = upload(&app, &theirs, (800, 600)).await;

    for list in [
        json!([one["id"]]),
        json!([one["id"], two["id"], alien["id"]]),
        json!([one["id"], alien["id"]]),
        json!([]),
    ] {
        let (status, _) = call(
            &app,
            request(
                "PATCH",
                "/api/v1/me/photos/order",
                Some(&mine),
                Some(json!({ "photo_ids": list })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "liste {list} acceptée");
    }
}

/// Le deck rend vingt cartes ; si les photos n'y sont pas, l'écran n'affiche
/// que des dégradés et rien ne le dit.
#[tokio::test]
async fn the_deck_carries_the_photos_of_its_candidates() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (them, them_id) = candidate(&app, "photo-deck-eux", PHOTOS, "man", Some(here)).await;
    upload(&app, &them, (800, 600)).await;

    let (me, _) = candidate(&app, "photo-deck-moi", PHOTOS, "woman", Some(here)).await;
    only_see_age(&app, &me, PHOTOS, "men").await;

    let card = deck_card(&app, &me, them_id).await.expect("la carte");
    let photos = card["photos"].as_array().expect("des photos");
    assert_eq!(photos.len(), 1);
    assert!(photos[0]["url"].as_str().unwrap().starts_with("https://"));
}

/// Supprimer son compte doit emporter ses photos : la page de confidentialité
/// le promet, et une photo restée servie après un départ est exactement ce
/// qu'elle promet de ne pas faire.
#[tokio::test]
async fn deleting_the_account_takes_the_photos_with_it() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "photo-depart").await;
    let photo = upload(&app, &token, (800, 600)).await;
    let id = photo["id"].as_str().unwrap().to_owned();

    let (status, _) = call(&app, request("DELETE", "/api/v1/me", Some(&token), None)).await;
    assert!(status.is_success(), "suppression du compte : {status}");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/photos/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
