import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

// The HTML pages live in src/ next to their code; the build output in dist/ stays flat
// (index.html, pet.html, ...) because the Rust side opens windows by those names.
const root = fileURLToPath(new URL("./src", import.meta.url));
const page = (file: string) => `${root}/${file}`;

export default defineConfig({
  root,
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: fileURLToPath(new URL("./dist", import.meta.url)),
    emptyOutDir: true,
    target: "es2022",
    rollupOptions: {
      input: {
        picker: page("index.html"),
        pet: page("pet.html"),
        credits: page("credits.html"),
        contact: page("contact.html"),
      },
    },
  },
  test: { environment: "node" },
});
