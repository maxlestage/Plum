import { Navigate, Route, Routes, useParams } from "react-router-dom";

import { DocumentHead } from "./components/DocumentHead";
import { Layout } from "./components/Layout";
import { detectLanguage, isLanguage, LanguageProvider } from "./i18n";
import type { PageKey } from "./i18n/routes";
import { pageForSlug, pathFor } from "./i18n/routes";
import { Home } from "./pages/Home";
import { Help, Privacy, Terms } from "./pages/Legal";

const views: Record<PageKey, () => JSX.Element> = {
  home: Home,
  terms: Terms,
  privacy: Privacy,
  help: Help,
};

/**
 * Resolves `/:language/:slug`.
 *
 * Language lives in the path rather than in state so that every version has
 * its own address: shareable, bookmarkable, and indexable one language at a
 * time. It also means a reload keeps the language a visitor chose without
 * storing anything on their machine.
 */
function LocalisedRoute() {
  const { language, slug } = useParams();

  if (!isLanguage(language)) {
    return <Navigate to={pathFor(detectLanguage(), "home")} replace />;
  }

  const page = pageForSlug(language, slug);

  // An unknown slug within a known language: send them to that language's
  // home page, not to the detected one — they asked for this language.
  if (page === undefined) {
    return <Navigate to={pathFor(language, "home")} replace />;
  }

  const View = views[page];

  return (
    <LanguageProvider value={language}>
      <DocumentHead page={page} />
      <Layout page={page}>
        <View />
      </Layout>
    </LanguageProvider>
  );
}

export function App() {
  return (
    <Routes>
      {/* The bare root picks a language from the browser once, then hands
          over to a real URL. */}
      <Route
        path="/"
        element={<Navigate to={pathFor(detectLanguage(), "home")} replace />}
      />
      <Route path="/:language" element={<LocalisedRoute />} />
      <Route path="/:language/:slug" element={<LocalisedRoute />} />
      <Route
        path="*"
        element={<Navigate to={pathFor(detectLanguage(), "home")} replace />}
      />
    </Routes>
  );
}
