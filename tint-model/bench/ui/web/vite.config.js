import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { svelte } from '@sveltejs/vite-plugin-svelte';
export default defineConfig({
  base: './',
  plugins: [react(), svelte()],
  build: { rollupOptions: { input: { react: 'react.html', 'react-stack': 'react-stack.html', svelte: 'svelte.html', vanilla: 'vanilla.html' } } },
});
