const STORAGE_KEY = 'nasty-theme';

function createTheme() {
	// Read persisted preference; fall back to system preference
	function getInitial(): 'dark' | 'light' {
		if (typeof localStorage !== 'undefined') {
			const stored = localStorage.getItem(STORAGE_KEY);
			if (stored === 'dark' || stored === 'light') return stored;
		}
		if (typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: light)').matches) {
			return 'light';
		}
		return 'dark';
	}

	const initial = getInitial();
	let current = $state<'dark' | 'light'>(initial);

	function apply(theme: 'dark' | 'light') {
		if (typeof document !== 'undefined') {
			document.documentElement.classList.toggle('dark', theme === 'dark');
		}
	}

	// Apply on first load
	apply(initial);

	return {
		get current() { return current; },
		get isDark() { return current === 'dark'; },
		toggle() {
			current = current === 'dark' ? 'light' : 'dark';
			apply(current);
			localStorage.setItem(STORAGE_KEY, current);
		},
		set(theme: 'dark' | 'light') {
			current = theme;
			apply(current);
			localStorage.setItem(STORAGE_KEY, theme);
		},
	};
}

export const theme = createTheme();
