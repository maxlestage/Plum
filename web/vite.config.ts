import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    // Served by the Rust dyno from this directory; the Dockerfile copies it
    // out of the Node stage.
    outDir: "dist",
    sourcemap: false,
  },
  server: {
    proxy: {
      // `npm run dev` talks to a local API without CORS getting involved.
      "/api": "http://127.0.0.1:8080",
    },
  },
});
