import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

const page = (file: string) => fileURLToPath(new URL(file, import.meta.url));

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        picker: page("./index.html"),
        pet: page("./pet.html"),
        credits: page("./credits.html"),
        contact: page("./contact.html"),
      },
    },
  },
  test: { environment: "node" },
});
