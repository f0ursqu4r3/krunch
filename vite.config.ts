import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

// @tauri-apps/cli sets TAURI_DEV_HOST when running on a device/emulator.
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [vue(), tailwindcss()],

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // Tauri expects a fixed port and fails if that port is not available.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // Don't watch the Rust side; Cargo handles that.
      ignored: ["**/src-tauri/**"],
    },
  },

  // Env vars starting with these prefixes are exposed to the client.
  envPrefix: ["VITE_", "TAURI_ENV_*"],
}));
