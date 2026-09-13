import { Navigate, Route, Routes } from "react-router-dom";

import { Layout } from "./components/Layout";
import { Home } from "./pages/Home";
import { Aide, Conditions, Confidentialite } from "./pages/Legal";

export function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/conditions" element={<Conditions />} />
        <Route path="/confidentialite" element={<Confidentialite />} />
        <Route path="/aide" element={<Aide />} />
        {/* Anything else is a mistyped link, not a page worth inventing. */}
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Layout>
  );
}
