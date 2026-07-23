import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

declare const process: {
  env: {
    TAURI_DEV_HOST?: string;
  };
};

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [vue()],
  assetsInclude: ["**/*.logo"],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
