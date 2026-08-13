import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri loads the built frontend from its custom tauri://localhost scheme.
// Relative asset URLs are required for the packaged app; root-relative `/assets`
// URLs work in Vite's dev server but render a blank window in the bundle.
export default defineConfig({ base: './', plugins: [react()] });
