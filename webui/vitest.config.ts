import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

// Kept separate from vite.config.ts so vitest's bundled vite copy doesn't
// clash with the SvelteKit plugin's plugin types under svelte-check.
// We bring in just the Svelte plugin (not full SvelteKit) so vitest can
// compile *.svelte.ts files that use $state and other runes.
export default defineConfig({
	plugins: [svelte()],
	test: {
		include: ['src/**/*.{test,spec}.ts'],
		setupFiles: ['./vitest.setup.ts']
	}
});
