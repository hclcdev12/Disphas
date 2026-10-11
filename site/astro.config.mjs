// Oblivio website (docs/site.md). Self-contained: nothing outside site/ is used.
import { defineConfig } from "astro/config";
import node from "@astrojs/node";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  site: "https://oblivio.eu",
  // Pages are static unless they opt out (`export const prerender = false`):
  // the language redirect of `/`, later the customer area and the license API.
  adapter: node({ mode: "standalone" }),
  i18n: {
    locales: ["en", "fr", "de"],
    defaultLocale: "en",
    routing: {
      prefixDefaultLocale: true,
      // `/` picks the browser language itself (src/pages/index.astro).
      redirectToDefaultLocale: false,
    },
  },
  vite: {
    plugins: [tailwindcss()],
  },
});
