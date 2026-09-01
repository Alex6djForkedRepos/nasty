import { beforeEach, describe, expect, test } from 'vitest';
import {
	DASHBOARD_PREFERENCES_KEY,
	LEGACY_DASHBOARD_PREFERENCES_KEY,
	LEGACY_DASHBOARD_V2_PREFERENCES_KEY,
	createDashboardView,
	dashboardPrefs,
	dashboardViewNameAvailable,
	dashboardWidgetIds,
	dashboardWidgetSupportsTiny,
	defaultDashboardPreferences,
	deleteDashboardView,
	getActiveDashboardView,
	initializeDashboardPreferences,
	loadDashboardPreferences,
	parseDashboardPreferences,
	renameDashboardView,
	resolveDashboardDensity,
	resolveDashboardWidgets,
	selectDashboardView,
	swapDashboardWidgets,
	updateActiveDashboardView,
} from './dashboard.svelte';

beforeEach(() => {
	localStorage.clear();
	dashboardPrefs.reset();
});

describe('dashboard preferences', () => {
	test('defaults missing, malformed, and unknown versions to overview', () => {
		expect(parseDashboardPreferences(null).preset).toBe('overview');
		expect(parseDashboardPreferences('{')).toEqual(defaultDashboardPreferences());
		expect(parseDashboardPreferences('{"version":4,"preset":"storage"}')).toEqual(defaultDashboardPreferences());
	});

	test('migrates the v1 custom layout without losing its configuration', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			version: 1,
			preset: 'custom',
			density: 'compact',
			widgets: [
				{ id: 'storage', visible: false, width: 'half' },
				{ id: 'alerts', visible: true, width: 'full' },
			],
		}));

		expect(parsed.version).toBe(3);
		expect(parsed.preset).toBe('custom');
		expect(parsed.customViews).toHaveLength(1);
		expect(getActiveDashboardView(parsed)).toMatchObject({
			name: 'Custom',
			density: 'compact',
		});
		expect(getActiveDashboardView(parsed).widgets[0]).toEqual({ id: 'storage', visible: false, width: 'half', presentation: 'standard' });
		expect(new Set(getActiveDashboardView(parsed).widgets.map((widget) => widget.id)).size).toBe(8);
	});

	test('migrates v2 named views to standard widget presentations', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			version: 2,
			preset: 'custom',
			activeViewId: 'daily',
			customViews: [{
				id: 'daily',
				name: 'Daily',
				density: 'compact',
				widgets: [
					{ id: 'alerts', visible: true, width: 'half' },
					{ id: 'history', visible: true, width: 'full' },
				],
			}],
		}));

		expect(parsed.version).toBe(3);
		expect(parsed.activeViewId).toBe('daily');
		expect(getActiveDashboardView(parsed)).toMatchObject({ name: 'Daily', density: 'compact' });
		expect(getActiveDashboardView(parsed).widgets.every((widget) => widget.presentation === 'standard')).toBe(true);
	});

	test('initializes v3 storage from the newest available legacy key without overwriting v3 preferences', () => {
		localStorage.clear();
		localStorage.setItem(LEGACY_DASHBOARD_PREFERENCES_KEY, JSON.stringify({
			version: 1,
			preset: 'overview',
			density: 'compact',
			widgets: [],
		}));
		localStorage.setItem(LEGACY_DASHBOARD_V2_PREFERENCES_KEY, JSON.stringify({
			version: 2,
			preset: 'storage',
			activeViewId: 'custom-1',
			customViews: defaultDashboardPreferences().customViews,
		}));
		expect(initializeDashboardPreferences(localStorage).preset).toBe('storage');
		expect(JSON.parse(localStorage.getItem(DASHBOARD_PREFERENCES_KEY) ?? '{}')).toMatchObject({
			version: 3,
			preset: 'storage',
		});

		const stored = defaultDashboardPreferences();
		stored.preset = 'monitoring';
		localStorage.setItem(DASHBOARD_PREFERENCES_KEY, JSON.stringify(stored));
		expect(initializeDashboardPreferences(localStorage).preset).toBe('monitoring');
		expect(loadDashboardPreferences(localStorage).preset).toBe('monitoring');
	});

	test('normalizes named views, active selection, and newly added widget ids', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			version: 3,
			preset: 'custom',
			activeViewId: ' ops ',
			customViews: [
				{
					id: 'ops',
					name: 'Operations',
					density: 'compact',
					widgets: [
						{ id: 'storage', visible: false, width: 'half', presentation: 'tiny' },
						{ id: 'alerts', visible: true, width: 'half', presentation: 'tiny' },
					],
				},
				{ id: 'ops', name: 'operations', widgets: [] },
			],
		}));

		expect(parsed.activeViewId).toBe('ops');
		expect(parsed.customViews.map((view) => view.id)).toEqual(['ops', 'custom-1']);
		expect(parsed.customViews.map((view) => view.name)).toEqual(['Operations', 'operations 2']);
		expect(new Set(parsed.customViews[0].widgets.map((widget) => widget.id)).size).toBe(8);
		expect(resolveDashboardWidgets(parsed)).not.toContainEqual(expect.objectContaining({ id: 'storage' }));
		expect(parsed.customViews[0].widgets.find((widget) => widget.id === 'storage')?.presentation).toBe('standard');
		expect(parsed.customViews[0].widgets.find((widget) => widget.id === 'alerts')?.presentation).toBe('tiny');
		expect(resolveDashboardDensity(parsed)).toBe('compact');
	});

	test('limits tiny presentation to widgets with a useful reduced form', () => {
		expect(dashboardWidgetIds.filter(dashboardWidgetSupportsTiny)).toEqual([
			'alerts', 'system', 'operations', 'history',
		]);
	});

	test('repairs a custom view with no visible widgets', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			version: 3,
			preset: 'custom',
			activeViewId: 'empty',
			customViews: [{
				id: 'empty',
				name: 'Empty',
				widgets: defaultDashboardPreferences().customViews[0].widgets.map((widget) => ({ ...widget, visible: false })),
			}],
		}));

		expect(getActiveDashboardView(parsed).widgets.filter((widget) => widget.visible)).toHaveLength(1);
	});

	test('resolves fixed presets independently from named custom views', () => {
		const preferences = defaultDashboardPreferences();
		preferences.preset = 'storage';
		preferences.customViews[0].widgets = preferences.customViews[0].widgets.map((widget) => ({ ...widget, visible: false }));

		expect(resolveDashboardWidgets(preferences).map((widget) => widget.id)).toEqual([
			'alerts', 'system', 'operations', 'storage',
		]);
		expect(resolveDashboardDensity(preferences)).toBe('compact');
	});

	test('creates, duplicates, renames, selects, and deletes named views', () => {
		let preferences = createDashboardView(defaultDashboardPreferences(), 'Storage ops');
		preferences = updateActiveDashboardView(preferences, {
			density: 'compact',
			widgets: getActiveDashboardView(preferences).widgets.map((widget) =>
				widget.id === 'history' ? { ...widget, presentation: 'tiny' } : widget
			),
		});
		const storageView = getActiveDashboardView(preferences);
		preferences = createDashboardView(preferences, 'Storage ops copy', storageView);
		const copyId = preferences.activeViewId;

		expect(getActiveDashboardView(preferences).density).toBe('compact');
		expect(getActiveDashboardView(preferences).widgets.find((widget) => widget.id === 'history')?.presentation).toBe('tiny');
		expect(dashboardViewNameAvailable(preferences, 'storage OPS')).toBe(false);
		preferences = renameDashboardView(preferences, copyId, 'Daily');
		expect(getActiveDashboardView(preferences).name).toBe('Daily');

		preferences = selectDashboardView(preferences, storageView.id);
		expect(getActiveDashboardView(preferences).name).toBe('Storage ops');
		preferences = deleteDashboardView(preferences, storageView.id);
		expect(preferences.customViews.some((view) => view.id === storageView.id)).toBe(false);
		expect(preferences.customViews.some((view) => view.id === preferences.activeViewId)).toBe(true);
	});

	test('does not delete the final custom view', () => {
		const preferences = defaultDashboardPreferences();
		expect(deleteDashboardView(preferences, preferences.activeViewId)).toBe(preferences);
	});

	test('updates only the active named view', () => {
		let preferences = createDashboardView(defaultDashboardPreferences(), 'Second');
		const first = preferences.customViews[0];
		preferences = updateActiveDashboardView(preferences, { density: 'compact' });

		expect(preferences.customViews.find((view) => view.id === first.id)?.density).toBe('comfortable');
		expect(getActiveDashboardView(preferences).density).toBe('compact');
	});

	test('persists the active named view in v3 storage', () => {
		const preferences = createDashboardView(defaultDashboardPreferences(), 'Monitoring desk');
		dashboardPrefs.set(preferences);

		expect(dashboardPrefs.value.preset).toBe('custom');
		expect(JSON.parse(localStorage.getItem(DASHBOARD_PREFERENCES_KEY) ?? '{}')).toMatchObject({
			version: 3,
			preset: 'custom',
			activeViewId: preferences.activeViewId,
		});
	});

	test('swaps custom widgets without mutating the saved view', () => {
		const preferences = defaultDashboardPreferences();
		const widgets = getActiveDashboardView(preferences).widgets;
		const swapped = swapDashboardWidgets(widgets, 'alerts', 'storage');

		expect(swapped.map((widget) => widget.id)).toEqual([
			'storage', 'system', 'summary', 'operations', 'alerts', 'history', 'network', 'disk_io',
		]);
		expect(widgets[0].id).toBe('alerts');
		expect(swapDashboardWidgets(widgets, 'alerts', 'alerts')).not.toBe(widgets);
	});
});
