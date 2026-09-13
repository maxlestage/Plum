import { Link } from "react-router-dom";

import { PlumMark } from "../components/PlumMark";

const steps = [
  {
    title: "Une photo, deux phrases",
    body: "Le strict minimum pour qu'on ait envie de répondre. Une photo est obligatoire ; le reste se remplit en trente secondes.",
  },
  {
    title: "On balaie",
    body: "À droite si oui, à gauche si non, vers le haut si vraiment. Le chevron ouvre le profil complet quand trois lignes ne suffisent pas à décider.",
  },
  {
    title: "On se parle, ou pas",
    body: "Un match ouvre une conversation. Personne n'est obligé d'y aller, et se retirer tient en deux gestes.",
  },
];

const principles = [
  {
    title: "Pas d'algorithme mystérieux",
    body: "Le deck trie par distance et par vos critères. C'est tout. Rien n'est vendu, rien n'est mis en avant contre paiement.",
  },
  {
    title: "Signaler est toujours à un geste",
    body: "Depuis une carte, depuis un profil complet, depuis une conversation. Bloquer retire la personne des deux côtés, immédiatement.",
  },
  {
    title: "Les distances restent vagues",
    body: "Moins d'un kilomètre, puis au kilomètre près, puis arrondies par tranches de cinq. Assez pour savoir si c'est le même quartier, jamais assez pour trouver quelqu'un.",
  },
  {
    title: "Utilisable sans voir l'écran",
    body: "Le deck se pilote au balayage, mais chaque verdict est aussi une action VoiceOver. Une application qu'on ne peut utiliser qu'à l'œil exclut du monde pour rien.",
  },
];

export function Home() {
  return (
    <>
      <section className="section" style={{ paddingTop: 72 }}>
        <div className="shell">
          <div className="prose">
            <PlumMark size={72} />
            <h1
              style={{
                fontSize: "clamp(2.2rem, 8vw, 3.4rem)",
                margin: "20px 0 12px",
              }}
            >
              Des rencontres pas sérieuses.
              <br />
              C'est déjà beaucoup.
            </h1>
            <p
              className="muted"
              style={{ fontSize: "1.15rem", margin: "0 0 28px" }}
            >
              Plum est une application iOS. Une photo, deux phrases, et on verra
              bien.
            </p>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
              <a className="button" href="#disponibilite">
                Quand est-ce disponible ?
              </a>
              <Link className="button button--quiet" to="/aide">
                Poser une question
              </Link>
            </div>
          </div>
        </div>
      </section>

      <section className="section">
        <div className="shell">
          <p className="eyebrow">Comment ça marche</p>
          <div className="grid grid--three">
            {steps.map((step, index) => (
              <article className="card" key={step.title}>
                <p
                  aria-hidden="true"
                  style={{
                    margin: 0,
                    fontWeight: 700,
                    fontSize: "1.6rem",
                    background: "var(--warm-gradient)",
                    WebkitBackgroundClip: "text",
                    backgroundClip: "text",
                    color: "transparent",
                  }}
                >
                  {index + 1}
                </p>
                <h2 style={{ fontSize: "1.15rem", margin: "8px 0" }}>
                  {step.title}
                </h2>
                <p className="muted" style={{ margin: 0 }}>
                  {step.body}
                </p>
              </article>
            ))}
          </div>
        </div>
      </section>

      <section className="section">
        <div className="shell">
          <p className="eyebrow">Ce qu'on a décidé</p>
          <div className="grid grid--two">
            {principles.map((principle) => (
              <article className="card" key={principle.title}>
                <h2 style={{ fontSize: "1.15rem", margin: "0 0 8px" }}>
                  {principle.title}
                </h2>
                <p className="muted" style={{ margin: 0 }}>
                  {principle.body}
                </p>
              </article>
            ))}
          </div>
        </div>
      </section>

      <section className="section" id="disponibilite">
        <div className="shell prose">
          <p className="eyebrow">Disponibilité</p>
          <h2 style={{ fontSize: "1.6rem", marginBottom: 12 }}>
            Pas encore téléchargeable
          </h2>
          <p className="muted">
            L'application iOS est écrite et testée, le serveur aussi, mais rien
            n'est encore passé par TestFlight. Autant le dire ici plutôt que de
            faire patienter devant un bouton qui ne mène nulle part.
          </p>
          <p className="muted">
            Il n'y a pas non plus de formulaire pour laisser son adresse : garder
            des adresses avant d'avoir quoi que ce soit à envoyer serait une
            collecte sans objet.
          </p>
        </div>
      </section>
    </>
  );
}
