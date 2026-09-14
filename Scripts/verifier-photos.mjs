// Vérifie les photos — envoi, réduction, service et suppression contre un déploiement réel.
//
// Les tests d'intégration montent le serveur eux-mêmes et parlent à une base
// de test ; celui-ci ne suppose rien. Il s'inscrit, agit, vérifie et efface
// ses comptes — contre l'adresse qu'on lui donne, la production comprise.
// C'est ce qui prouve l'assemblage : l'image, la configuration, le routeur
// d'hébergement, la base.
//
//   cd Scripts && npm install        # une fois
//   node verifier-photos.mjs
//   PLUM_HOST=127.0.0.1:8080 PLUM_TLS=0 node verifier-photos.mjs   # contre un serveur local

import zlib from "node:zlib";

const HOST = process.env.PLUM_HOST ?? "plum-a5f3c7189761.herokuapp.com";
const TLS = process.env.PLUM_TLS !== "0";
const BASE = `${TLS ? "https" : "http"}://${HOST}/api/v1`;
const fails = [];

const check = (label, got, want) => {
  const ok = JSON.stringify(got) === JSON.stringify(want);
  console.log((ok ? "  ok   " : "FAIL  ") + label +
    (ok ? "" : ` — attendu ${JSON.stringify(want)}, reçu ${JSON.stringify(got)}`));
  if (!ok) fails.push(label);
};

function crc32(buf) {
  let c, table = [];
  for (let n = 0; n < 256; n++) {
    c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  let crc = 0xffffffff;
  for (const b of buf) crc = table[(crc ^ b) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

/// Un PNG RGB avec du détail, pour qu'il ne se comprime pas à rien.
function png(width, height) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;   // 8 bits par canal
  ihdr[9] = 2;   // couleur vraie, sans alpha
  const rows = [];
  for (let y = 0; y < height; y++) {
    const row = Buffer.alloc(1 + width * 3);
    row[0] = 0; // filtre « none »
    for (let x = 0; x < width; x++) {
      row[1 + x * 3] = (x * 7) % 256;
      row[2 + x * 3] = (y * 5) % 256;
      row[3 + x * 3] = (x + y) % 256;
    }
    rows.push(row);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib.deflateSync(Buffer.concat(rows))),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

async function call(method, path, token, body) {
  const headers = {};
  if (token) headers.authorization = `Bearer ${token}`;
  if (body !== undefined) headers["content-type"] = "application/json";
  const r = await fetch(BASE + path, {
    method, headers, body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await r.text();
  return [r.status, text ? JSON.parse(text) : null];
}

async function upload(token, bytes, filename = "photo.png") {
  const boundary = "plum." + crypto.randomUUID();
  const head = Buffer.from(
    `--${boundary}\r\nContent-Disposition: form-data; name="file"; ` +
    `filename="${filename}"\r\nContent-Type: application/octet-stream\r\n\r\n`);
  const tail = Buffer.from(`\r\n--${boundary}--\r\n`);
  const r = await fetch(`${BASE}/me/photos`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": `multipart/form-data; boundary=${boundary}`,
    },
    body: Buffer.concat([head, bytes, tail]),
  });
  const text = await r.text();
  return [r.status, text ? JSON.parse(text) : null];
}

/// Une inscription, ou un arrêt qui dit pourquoi.
///
/// Ces vérifications créent plusieurs comptes par exécution, depuis une seule
/// adresse IP — et l'inscription est limitée à dix par heure et par adresse.
/// Lancées trois fois de suite, elles se heurtent donc à leur propre
/// garde-fou. Sans ce contrôle, l'échec arrivait sous la forme d'un
/// « cannot read properties of undefined », ce qui n'aide personne.
/*
 * Les comptes créés par cette vérification, et leur effacement garanti.
 *
 * Ils étaient effacés à la fin du script. Une vérification qui échoue en
 * route sautait donc le nettoyage et laissait ses comptes dans le deck des
 * vrais utilisateurs — c'est exactement ce qu'on a retrouvé en production,
 * deux profils orphelins d'une exécution interrompue.
 *
 * Le nettoyage est maintenant accroché à la fin du processus, quelle qu'en
 * soit la cause : succès, échec d'une vérification, ou exception.
 */
const aEffacer = [];
let nettoye = false;

async function nettoyer() {
  if (nettoye) return;
  nettoye = true;
  for (const t of aEffacer) {
    try {
      await call("DELETE", "/me", t);
    } catch {
      // Un compte qu'on n'arrive pas à effacer ne doit pas empêcher
      // d'effacer les autres.
    }
  }
  if (aEffacer.length) console.log("  (comptes de test supprimés)");
}

process.on("beforeExit", nettoyer);
process.on("uncaughtException", async (e) => {
  console.error("\nInterrompu :", e?.message ?? e);
  await nettoyer();
  process.exit(1);
});

async function signup(name) {
  const email = `photo-${crypto.randomUUID().slice(0, 12)}@plum.app`;
  const [status, b] = await call("POST", "/auth/sign-up", null, {
    email, password: crypto.randomUUID(), display_name: name,
    birth_date: "1996-04-12T00:00:00Z", gender: "woman",
  });
  if (status === 429) {
    console.error(
      "\nArrêt : l'inscription est limitée à dix par heure et par adresse IP.\n" +
      "        C'est le garde-fou qui fonctionne, pas une panne. Réessayez plus\n" +
      "        tard, ou visez un serveur local avec PLUM_HOST et PLUM_TLS=0.",
    );
    process.exit(2);
  }
  if (status !== 200 || !b?.tokens) {
    console.error(`\nArrêt : l'inscription a répondu ${status} — ${JSON.stringify(b)}`);
    process.exit(2);
  }
  aEffacer.push(b.tokens.access_token);
  return [b.tokens.access_token, b.user.id];
}

const [token] = await signup("Camille");

// --- l'envoi ---
const [status, photo] = await upload(token, png(1800, 1350));
check("l'envoi est accepté", status, 200);
check("la première photo est la couverture", photo?.position, 0);
const url = photo?.url ?? "";
check("l'adresse est absolue", url.startsWith(`https://${HOST}/photos/`), true);

// --- les octets, sans jeton ---
const image = await fetch(url);
check("les octets se récupèrent sans être connecté", image.status, 200);
check("servis en JPEG", image.headers.get("content-type"), "image/jpeg");
check("mis en cache pour toujours",
  (image.headers.get("cache-control") || "").includes("immutable"), true);

const bytes = Buffer.from(await image.arrayBuffer());
check("c'est bien un JPEG", [bytes[0], bytes[1]], [0xff, 0xd8]);
// La largeur se lit dans le segment SOF0 du JPEG.
let width = null, height = null;
for (let i = 2; i < bytes.length - 9; i++) {
  if (bytes[i] === 0xff && (bytes[i + 1] === 0xc0 || bytes[i + 1] === 0xc2)) {
    height = bytes.readUInt16BE(i + 5);
    width = bytes.readUInt16BE(i + 7);
    break;
  }
}
check("réduite au grand côté", width, 1200);
check("proportions gardées", height, 900);
console.log(`  ·      ${Math.round(png(1800, 1350).length / 1024)} Kio envoyés → ${Math.round(bytes.length / 1024)} Kio stockés`);

// --- le profil la porte ---
const [, profile] = await call("GET", "/me/profile", token);
check("le profil porte la photo", profile?.photos?.length, 1);
check("avec la même adresse", profile?.photos?.[0]?.url, url);

// --- ce qui n'est pas une image ---
const [refused, why] = await upload(token, Buffer.from("MZ\x90\x00 pas une image"), "x.jpg");
check("un fichier qui n'est pas une image est refusé", refused, 400);
check("et le message dit quoi faire", (why?.message || "").includes("HEIC"), true);

// --- la septième ---
for (let i = 1; i < 6; i++) {
  const [s] = await upload(token, png(600, 450));
  if (s !== 200) { check(`photo ${i + 1}`, s, 200); break; }
}
const [seventh] = await upload(token, png(600, 450));
check("la septième est refusée", seventh, 400);

// --- le départ emporte les photos ---
//
// La suppression du compte est ici une *étape de la vérification*, pas du
// nettoyage : c'est elle que la ligne suivante contrôle. La confondre avec le
// nettoyage de fin de script — ce que j'ai fait une fois — repousse la
// suppression après l'assertion, qui échoue alors sans rien signaler de vrai.
await call("DELETE", "/me", token);
const gone = await fetch(url);
check("la photo part avec le compte", gone.status, 404);

console.log(fails.length ? `\n${fails.length} ÉCHEC(S)` : "\nTOUT VERT");
process.exit(fails.length ? 1 : 0);
