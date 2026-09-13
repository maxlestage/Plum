import { Link } from "react-router-dom";

import { PlumMark } from "../components/PlumMark";
import { useCopy, useLanguage } from "../i18n";
import { pathFor } from "../i18n/routes";

export function Home() {
  const copy = useCopy();
  const language = useLanguage();

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
              {copy.hero.headlineTop}
              <br />
              {copy.hero.headlineBottom}
            </h1>
            <p
              className="muted"
              style={{ fontSize: "1.15rem", margin: "0 0 28px" }}
            >
              {copy.hero.tagline}
            </p>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
              <a className="button" href="#availability">
                {copy.hero.availabilityCta}
              </a>
              <Link
                className="button button--quiet"
                to={pathFor(language, "help")}
              >
                {copy.hero.questionCta}
              </Link>
            </div>
          </div>
        </div>
      </section>

      <section className="section">
        <div className="shell">
          <p className="eyebrow">{copy.stepsEyebrow}</p>
          <div className="grid grid--three">
            {copy.steps.map((step, index) => (
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
          <p className="eyebrow">{copy.principlesEyebrow}</p>
          <div className="grid grid--two">
            {copy.principles.map((principle) => (
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

      <section className="section" id="availability">
        <div className="shell prose">
          <p className="eyebrow">{copy.availability.eyebrow}</p>
          <h2 style={{ fontSize: "1.6rem", marginBottom: 12 }}>
            {copy.availability.title}
          </h2>
          <p className="muted">{copy.availability.body}</p>
          <p className="muted">{copy.availability.noMailingList}</p>
          {/*
           * Only shown where it is true: the app is French-only, so the
           * translated pages say so rather than let someone find out after
           * downloading.
           */}
          {copy.availability.appLanguageNotice !== null && (
            <p className="muted">
              <strong>{copy.availability.appLanguageNotice}</strong>
            </p>
          )}
        </div>
      </section>
    </>
  );
}
