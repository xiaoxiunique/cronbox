import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5188,
    strictPort: true,
    proxy: {
      "/api": "http://127.0.0.1:4317",
    },
  },
});
