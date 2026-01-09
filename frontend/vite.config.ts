import { defineConfig } from "vite";
import { fresh } from "@fresh/plugin-vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [fresh(), tailwindcss()],
  server: {
    port: Deno.env.get("PORT") ? Number(Deno.env.get("PORT")) : 8000
  }
});
