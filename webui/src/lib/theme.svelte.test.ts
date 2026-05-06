import { beforeEach, describe, expect, test } from 'vitest';
import { theme } from './theme.svelte';

beforeEach(() => {
	// Each test starts from a known state. theme is a module-level singleton,
	// so we explicitly set it rather than reload the module.
	theme.set('dark');
	localStorage.removeItem('nasty-theme');
});

describe('theme', () => {
	test('set("light") updates current and isDark', () => {
		theme.set('light');
		expect(theme.current).toBe('light');
		expect(theme.isDark).toBe(false);
	});

	test('set("dark") updates current and isDark', () => {
		theme.set('light');
		theme.set('dark');
		expect(theme.current).toBe('dark');
		expect(theme.isDark).toBe(true);
	});

	test('toggle flips dark → light → dark', () => {
		theme.set('dark');
		theme.toggle();
		expect(theme.current).toBe('light');
		theme.toggle();
		expect(theme.current).toBe('dark');
	});

	test('set persists to localStorage', () => {
		theme.set('light');
		expect(localStorage.getItem('nasty-theme')).toBe('light');
		theme.set('dark');
		expect(localStorage.getItem('nasty-theme')).toBe('dark');
	});

	test('toggle persists the new value to localStorage', () => {
		theme.set('dark');
		theme.toggle();
		expect(localStorage.getItem('nasty-theme')).toBe('light');
	});
});
