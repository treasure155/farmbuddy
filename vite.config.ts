import { defineConfig } from 'vite';
import legacy from '@vitejs/plugin-legacy';

// Android phones can have an old built-in browser engine (System WebView), especially
// Android 7–9 phones that never update it. For Android builds the app is compiled to
// old-style JavaScript with the missing features added (polyfills), so it runs on every
// Android version the app installs on (7.0 and newer).
const platform = process.env.TAURI_ENV_PLATFORM;
const android = platform === 'android';

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // The Rust side is rebuilt by Tauri itself. Vite must not watch it: on Windows the
    // files cargo is writing in src-tauri/target are locked, and watching them stops
    // `npm run tauri dev` with "EBUSY: resource busy or locked".
    watch: { ignored: ['**/src-tauri/**', '**/node_modules/**', '**/dist/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  plugins: android ? [legacy({ targets: ['chrome >= 49', 'android >= 5'], renderModernChunks: false })] : [],
  build: {
    target: platform === 'windows' ? 'chrome105' : 'safari13',
    // One file instead of a file per page: pages can never fail to load on a slow or old phone.
    rollupOptions: { output: { inlineDynamicImports: true } },
  },
});
