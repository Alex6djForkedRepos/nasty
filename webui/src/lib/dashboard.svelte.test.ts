import { beforeEach, describe, expect, test } from 'vitest';
import {
	DASHBOARD_PREFERENCES_KEY,
	dashboardPrefs,
	defaultDashboardPreferences,
	parseDashboardPreferences,
	resolveDashboardDensity,
	resolveDashboardWidgets,
	swapDashboardWidgets,
} from './dashboard.svelte';

beforeEach(() => {
	localStorage.clear();
	dashboardPrefs.reset();
});

describe('dashboard preferences', () => {
	test('defaults missing, malformed, and unknown versions to overview', () => {
		expect(parseDashboardPreferences(null).preset).toBe('overview');
		expect(parseDashboardPreferences('{')).toEqual(defaultDashboardPreferences());
		expect(parseDashboardPreferences('{"version":2,"preset":"storage"}')).toEqual(defaultDashboardPreferences());
	});

	test('normalizes custom widgets without losing newly added widget ids', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			version: 1,
			preset: 'custom',
			density: 'compact',
			widgets: [
				{ id: 'storage', visible: false, width: 'half' },
				{ id: 'storage', visible: true, width: 'full' },
				{ id: 'unknown', visible: true, width: 'full' },
			],
		}));

		expect(parsed.widgets[0]).toEqual({ id: 'storage', visible: false, width: 'half' });
		expect(new Set(parsed.widgets.map((widget) => widget.id)).size).toBe(8);
		expect(resolveDashboardWidgets(parsed)).not.toContainEqual(expect.objectContaining({ id: 'storage' }));
		expect(resolveDashboardDensity(parsed)).toBe('compact');
	});

	test('resolves fixed presets independently from custom widget choices', () => {
		const preferences = defaultDashboardPreferences();
		preferences.preset = 'storage';
		preferences.widgets = preferences.widgets.map((widget) => ({ ...widget, visible: false }));

		expect(resolveDashboardWidgets(preferences).map((widget) => widget.id)).toEqual([
			'alerts', 'system', 'operations', 'storage',
		]);
		expect(resolveDashboardDensity(preferences)).toBe('compact');
	});

	test('persists a versioned custom layout', () => {
		const preferences = defaultDashboardPreferences();
		preferences.preset = 'custom';
		preferences.density = 'compact';
		preferences.widgets[0].visible = false;
		dashboardPrefs.set(preferences);

		expect(dashboardPrefs.value.preset).toBe('custom');
		expect(JSON.parse(localStorage.getItem(DASHBOARD_PREFERENCES_KEY) ?? '{}')).toMatchObject({
			version: 1,
			preset: 'custom',
			density: 'compact',
		});
	});

	test('swaps custom widgets without mutating the saved layout', () => {
		const preferences = defaultDashboardPreferences();
		const swapped = swapDashboardWidgets(preferences.widgets, 'alerts', 'storage');

		expect(swapped.map((widget) => widget.id)).toEqual([
			'storage', 'system', 'summary', 'operations', 'alerts', 'history', 'network', 'disk_io',
		]);
		expect(preferences.widgets[0].id).toBe('alerts');
		expect(swapDashboardWidgets(preferences.widgets, 'alerts', 'alerts')).not.toBe(preferences.widgets);
	});
});
