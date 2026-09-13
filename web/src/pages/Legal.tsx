import type { Entry } from "../i18n";
import { useCopy } from "../i18n";

/**
 * These pages describe what the application actually does — that part is
 * accurate, because it was written from the code. What they are not is legal
 * advice, and a dating app is a bad place to improvise: the search criteria
 * alone let someone infer sexual orientation, which the GDPR treats as a
 * special category. Hence the banner, which stays until a lawyer has read
 * these — in all three languages, since a translated draft is still a draft.
 */
function Draft() {
  const copy = useCopy();

  return (
    <div
      className="card"
      style={{
        borderColor: "var(--apricot)",
        background: "color-mix(in srgb, var(--apricot) 12%, var(--surface))",
        marginBottom: 28,
      }}
    >
      <strong>{copy.draft.heading}</strong>
      <p style={{ margin: "8px 0 0" }}>{copy.draft.body}</p>
    </div>
  );
}

function Page({ title, sections }: { title: string; sections: Entry[] }) {
  return (
    <section className="section">
      <div className="shell prose">
        <h1
          style={{ fontSize: "clamp(1.8rem, 6vw, 2.4rem)", marginBottom: 20 }}
        >
          {title}
        </h1>
        <Draft />
        {sections.map((section) => (
          <div key={section.title}>
            <h2 style={{ fontSize: "1.2rem", marginTop: 28 }}>
              {section.title}
            </h2>
            <p className="muted">{section.body}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

export function Terms() {
  const copy = useCopy();
  return <Page title={copy.terms.title} sections={copy.terms.sections} />;
}

export function Privacy() {
  const copy = useCopy();
  return <Page title={copy.privacy.title} sections={copy.privacy.sections} />;
}

export function Help() {
  const copy = useCopy();
  return <Page title={copy.help.title} sections={copy.help.sections} />;
}
