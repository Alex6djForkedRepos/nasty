// Browser-globals stubs for vitest's node environment.
//
// rpc.ts and theme.svelte.ts both touch `localStorage` at module-import time.
// Vitest's node env provides a partial localStorage without getItem, so the
// import would throw before any test runs. We back it with a real Map so
// theme tests can verify persistence too.
const store = new Map<string, string>();
globalThis.localStorage = {
	getItem: (k: string) => store.get(k) ?? null,
	setItem: (k: string, v: string) => {
		store.set(k, v);
	},
	removeItem: (k: string) => {
		store.delete(k);
	},
	clear: () => {
		store.clear();
	},
	key: (i: number) => Array.from(store.keys())[i] ?? null,
	get length() {
		return store.size;
	}
};
