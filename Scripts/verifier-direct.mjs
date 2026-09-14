// Vérifie le direct — le socket, ses évènements et ses garde-fous contre un déploiement réel.
//
// Les tests d'intégration montent le serveur eux-mêmes et parlent à une base
// de test ; celui-ci ne suppose rien. Il s'inscrit, agit, vérifie et efface
// ses comptes — contre l'adresse qu'on lui donne, la production comprise.
// C'est ce qui prouve l'assemblage : l'image, la configuration, le routeur
// d'hébergement, la base.
//
//   cd Scripts && npm install        # une fois
//   node verifier-direct.mjs
//   PLUM_HOST=127.0.0.1:8080 PLUM_TLS=0 node verifier-direct.mjs   # contre un serveur local

import WebSocket from "ws";

const HOST = process.env.PLUM_HOST ?? "plum-a5f3c7189761.herokuapp.com";
const TLS = process.env.PLUM_TLS !== "0";
const BASE = `${TLS ? "https" : "http"}://${HOST}/api/v1`;
const fails = [];

const check = (label, got, want) => {
  const ok = JSON.stringify(got) === JSON.stringify(want);
  console.log((ok ? "  ok   " : "FAIL  ") + label + (ok ? "" : ` — attendu ${JSON.stringify(want)}, reçu ${JSON.stringify(got)}`));
  if (!ok) fails.push(label);
};

async function call(method, path, token, body) {
  const headers = {};
  if (token) headers.authorization = `Bearer ${token}`;
  if (body !== undefined) headers["content-type"] = "application/json";
  const r = await fetch(BASE + path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
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

async function signup(name, gender) {
  const email = `ws-${crypto.randomUUID().slice(0, 12)}@plum.app`;
  const [status, b] = await call("POST", "/auth/sign-up", null, {
    email, password: crypto.randomUUID(), display_name: name,
    birth_date: "1996-04-12T00:00:00Z", gender,
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

/// Ouvre un socket et rend une fonction qui attend le prochain évènement.
function open(token) {
  const socket = new WebSocket(`${TLS ? "wss" : "ws"}://${HOST}/ws`, { headers: { authorization: `Bearer ${token}` } });
  const queue = [];
  const waiting = [];
  socket.on("message", (data) => {
    const event = JSON.parse(data.toString());
    if (waiting.length) waiting.shift()(event);
    else queue.push(event);
  });
  const ready = new Promise((resolve, reject) => {
    socket.once("open", resolve);
    socket.once("error", reject);
  });
  const next = (patience = 8000) =>
    new Promise((resolve) => {
      if (queue.length) return resolve(queue.shift());
      // Le rappel doit partir avec le délai. Laissé en place, il avalait
      // l'évènement suivant en le remettant à une promesse déjà tenue — et
      // chaque vérification « rien ne doit arriver » faisait disparaître le
      // vrai évènement qui la suivait.
      const slot = (event) => { clearTimeout(timer); resolve(event); };
      const timer = setTimeout(() => {
        const at = waiting.indexOf(slot);
        if (at >= 0) waiting.splice(at, 1);
        resolve(null);
      }, patience);
      waiting.push(slot);
    });
  return { socket, ready, next, send: (o) => socket.send(Buffer.from(JSON.stringify(o))) };
}

const [at, ai] = await signup("Camille", "woman");
const [bt, bi] = await signup("Dominique", "man");

// --- l'authentification ---
const anonymous = new WebSocket(`${TLS ? "wss" : "ws"}://${HOST}/ws`);
const refused = await new Promise((resolve) => {
  anonymous.once("error", () => resolve("refusé"));
  anonymous.once("open", () => { anonymous.close(); resolve("ouvert"); });
});
check("un socket sans jeton est refusé", refused, "refusé");

// --- le match poussé ---
await call("POST", "/discovery/swipes", at, { target_profile_id: bi, decision: "like" });
const waiting = open(at);
await waiting.ready;
check("le socket authentifié s'ouvre", waiting.socket.readyState, WebSocket.OPEN);

await call("POST", "/discovery/swipes", bt, { target_profile_id: ai, decision: "like" });
const matched = await waiting.next();
check("le match arrive en direct", matched?.type, "match");
check("et porte le profil de l'autre", matched?.match?.profile?.id, bi);

const [, m] = await call("GET", "/matches", at);
const matchId = m.items[0].id;
const [, conv] = await call("POST", `/matches/${matchId}/conversation`, at);

// --- le message poussé ---
const listener = open(bt);
await listener.ready;
const [, written] = await call("POST", `/conversations/${conv.id}/messages`, at,
  { client_id: crypto.randomUUID(), body: "tu fais quoi ce soir ?" });
const pushed = await listener.next();
check("le message arrive en direct", pushed?.type, "message");
check("avec son texte", pushed?.message?.body, "tu fais quoi ce soir ?");
check("et le même identifiant que l'API", pushed?.message?.id, written.id);

// --- l'expéditeur n'entend pas son propre écho ---
check("pas d'écho vers l'expéditeur", await waiting.next(1500), null);

// --- la frappe ---
listener.send({ type: "typing", conversation_id: conv.id });
const typing = await waiting.next();
check("la frappe est relayée", typing?.type, "typing");
check("sur la bonne conversation", typing?.conversation_id, conv.id);

// --- l'accusé de lecture ---
await call("POST", `/conversations/${conv.id}/read`, bt);
const read = await waiting.next();
check("l'accusé de lecture remonte", read?.type, "read");
check("sur le bon message", read?.message_id, written.id);

// --- un intrus ---
const [st] = await signup("Etranger", "woman");
const intruder = open(st);
await intruder.ready;
intruder.send({ type: "typing", conversation_id: conv.id });
check("un intrus ne fait clignoter personne", await waiting.next(1500), null);
check("ni de l'autre côté", await listener.next(1500), null);

for (const s of [waiting, listener, intruder]) s.socket.close();

console.log(fails.length ? `\n${fails.length} ÉCHEC(S)` : "\nTOUT VERT");
process.exit(fails.length ? 1 : 0);
