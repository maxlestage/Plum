import type { ReactNode } from "react";

/**
 * These pages describe what the application actually does — that part is
 * accurate, because it was written from the code. What they are not is legal
 * advice, and a dating app is a bad place to improvise: the search criteria
 * alone let someone infer sexual orientation, which the GDPR treats as a
 * special category. Hence the banner, which stays until a lawyer has read
 * these.
 */
function Draft() {
  return (
    <div
      className="card"
      style={{
        borderColor: "var(--apricot)",
        background: "color-mix(in srgb, var(--apricot) 12%, var(--surface))",
        marginBottom: 28,
      }}
    >
      <strong>Brouillon, pas un document juridique.</strong>
      <p style={{ margin: "8px 0 0" }}>
        Ce texte décrit fidèlement ce que fait l'application, mais il n'a pas
        été relu par un juriste. Il doit l'être avant toute mise en service :
        une application de rencontres traite des données que le RGPD range
        parmi les catégories particulières.
      </p>
    </div>
  );
}

function Page({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="section">
      <div className="shell prose">
        <h1 style={{ fontSize: "clamp(1.8rem, 6vw, 2.4rem)", marginBottom: 20 }}>
          {title}
        </h1>
        <Draft />
        {children}
      </div>
    </section>
  );
}

export function Conditions() {
  return (
    <Page title="Conditions d'utilisation">
      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>Qui peut s'inscrire</h2>
      <p className="muted">
        Les personnes majeures uniquement. L'âge est demandé à l'inscription et
        vérifié par le serveur, pas seulement par l'application : un contrôle
        côté client est une courtoisie, pas un contrôle.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>Ce qu'on attend</h2>
      <p className="muted">
        Que le profil soit le vôtre, que les photos soient de vous, et que les
        conversations restent supportables. Le harcèlement, l'usurpation
        d'identité et les photos de mineurs entraînent une suppression sans
        préavis.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>Signalement</h2>
      <p className="muted">
        Chaque profil peut être signalé ou bloqué depuis une carte, depuis le
        profil complet et depuis la conversation. Bloquer retire la personne des
        deux côtés immédiatement.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>Fin du compte</h2>
      <p className="muted">
        La suppression du compte se fait depuis les réglages de l'application,
        sans passer par nous. Elle emporte les matchs, les messages et les
        photos.
      </p>
    </Page>
  );
}

export function Confidentialite() {
  return (
    <Page title="Confidentialité">
      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>
        Ce qui est collecté
      </h2>
      <p className="muted">
        Une adresse email, un mot de passe, un prénom, une date de naissance, un
        genre, et ce que vous écrivez : bio, ville, centres d'intérêt, photos,
        messages. Plus la position, si vous l'autorisez.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>La position</h2>
      <p className="muted">
        Elle sert à calculer des distances, et rien d'autre. Les distances
        affichées sont volontairement vagues : « moins d'1 km », puis au
        kilomètre près, puis arrondies par tranches de cinq au-delà de dix. Assez
        pour savoir si c'est le même quartier, jamais assez pour trouver
        quelqu'un. Refuser la localisation masque les distances et ne casse rien
        d'autre.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>Les mots de passe</h2>
      <p className="muted">
        Stockés hachés en Argon2id, jamais en clair. Les jetons qui gardent une
        session ouverte ne sont pas stockés non plus : seule leur empreinte
        l'est, pour qu'une base dérobée ne distribue pas de sessions.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>
        Données particulières
      </h2>
      <p className="muted">
        Les critères de recherche permettent d'inférer une orientation sexuelle,
        que le RGPD range parmi les catégories particulières de l'article 9.
        C'est précisément le point qui exige une relecture juridique, et la
        raison pour laquelle cette page reste un brouillon.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>Suppression</h2>
      <p className="muted">
        Depuis les réglages de l'application, à tout moment, sans demande à
        formuler.
      </p>
    </Page>
  );
}

export function Aide() {
  return (
    <Page title="Aide et contact">
      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>Un problème ?</h2>
      <p className="muted">
        Il n'y a pas encore d'adresse de contact publiée, et en inventer une qui
        ne serait pas relevée serait pire que rien. Elle apparaîtra ici avant la
        première mise à disposition sur TestFlight.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>
        Signaler quelqu'un
      </h2>
      <p className="muted">
        Depuis l'application, sur la carte, le profil complet ou la conversation.
        C'est plus rapide et mieux tracé que par écrit.
      </p>

      <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>État du service</h2>
      <p className="muted">
        L'API répond sur <code>/health</code>. Si l'application se plaint d'un
        problème de connexion, c'est le premier endroit à regarder.
      </p>
    </Page>
  );
}
