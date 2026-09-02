import { beforeEach, describe, expect, test } from 'vitest';
import {
	DASHBOARD_PREFERENCES_KEY,
	LEGACY_DASHBOARD_PREFERENCES_KEY,
	LEGACY_DASHBOARD_V2_PREFERENCES_KEY,
	LEGACY_DASHBOARD_V3_PREFERENCES_KEY,
	LEGACY_DASHBOARD_V4_PREFERENCES_KEY,
	LEGACY_DASHBOARD_V5_PREFERENCES_KEY,
	createDashboardView,
	dashboardPrefs,
	dashboardPresetTabVisible,
	dashboardViewNameAvailable,
	dashboardWidgetIds,
	dashboardWidgetSnapColumn,
	dashboardWidgetSupportsTiny,
	dashboardWidgetSupportsNarrowWidth,
	dashboardWidgetValidColumns,
	dashboardWidgetWidthClass,
	defaultDashboardPreferences,
	deleteDashboardView,
	getActiveDashboardView,
	initializeDashboardPreferences,
	layoutDashboardWidgets,
	loadDashboardPreferences,
	parseDashboardPreferences,
	placeDashboardWidget,
	renameDashboardView,
	resolveDashboardDensity,
	resolveDashboardWidgets,
	selectDashboardView,
	setDashboardPresetTabVisible,
	updateActiveDashboardView,
	type DashboardWidgetConfig,
} from './dashboard.svelte';

beforeEach(() => {
	localStorage.clear();
	dashboardPrefs.reset();
});

describe('dashboard preferences', () => {
	test('defaults missing, malformed, and unknown versions to overview', () => {
		expect(parseDashboardPreferences(null).preset).toBe('overview');
		expect(parseDashboardPreferences('{')).toEqual(defaultDashboardPreferences());
		expect(parseDashboardPreferences('{"version":7,"preset":"storage"}')).toEqual(defaultDashboardPreferences());
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

		expect(parsed.version).toBe(6);
		expect(parsed.preset).toBe('custom');
		expect(parsed.customViews).toHaveLength(1);
		expect(getActiveDashboardView(parsed)).toMatchObject({
			name: 'Custom',
			density: 'compact',
		});
		expect(getActiveDashboardView(parsed).widgets[0]).toEqual({ id: 'storage', visible: false, width: 'half', presentation: 'standard', column: 0, row: 0, priority: 0 });
		expect(new Set(getActiveDashboardView(parsed).widgets.map((widget) => widget.id)).size).toBe(13);
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

		expect(parsed.version).toBe(6);
		expect(parsed.activeViewId).toBe('daily');
		expect(getActiveDashboardView(parsed)).toMatchObject({ name: 'Daily', density: 'compact' });
		expect(getActiveDashboardView(parsed).widgets.every((widget) => widget.presentation === 'standard')).toBe(true);
	});

	test('initializes v6 storage from the newest available legacy key without overwriting v6 preferences', () => {
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
		const { hiddenPresetTabs: _, ...shippedV3 } = defaultDashboardPreferences();
		localStorage.setItem(LEGACY_DASHBOARD_V3_PREFERENCES_KEY, JSON.stringify({
			...shippedV3,
			version: 3,
			preset: 'monitoring',
		}));
		localStorage.setItem(LEGACY_DASHBOARD_V4_PREFERENCES_KEY, JSON.stringify({
			version: 4,
			preset: 'custom',
			hiddenPresetTabs: [],
			activeViewId: 'legacy',
			customViews: [{
				id: 'legacy',
				name: 'Legacy',
				density: 'comfortable',
				widgets: [{ id: 'health', visible: true, width: 'full', presentation: 'standard' }],
			}],
		}));
		localStorage.setItem(LEGACY_DASHBOARD_V5_PREFERENCES_KEY, JSON.stringify({
			...defaultDashboardPreferences(),
			version: 5,
			preset: 'monitoring',
		}));
		expect(initializeDashboardPreferences(localStorage).preset).toBe('monitoring');
		expect(JSON.parse(localStorage.getItem(DASHBOARD_PREFERENCES_KEY) ?? '{}')).toMatchObject({
			version: 6,
			preset: 'monitoring',
		});

		const stored = defaultDashboardPreferences();
		stored.preset = 'storage';
		localStorage.setItem(DASHBOARD_PREFERENCES_KEY, JSON.stringify(stored));
		expect(initializeDashboardPreferences(localStorage).preset).toBe('storage');
		expect(loadDashboardPreferences(localStorage).preset).toBe('storage');
	});

	test('expands v4 health and resource groups in place without losing configuration', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			version: 4,
			preset: 'custom',
			hiddenPresetTabs: ['overview'],
			activeViewId: 'legacy',
			customViews: [{
				id: 'legacy',
				name: 'Legacy',
				density: 'compact',
				widgets: [
					{ id: 'alerts', visible: true, width: 'full', presentation: 'tiny' },
					{ id: 'health', visible: false, width: 'half', presentation: 'standard' },
					{ id: 'network', visible: true, width: 'half', presentation: 'standard' },
					{ id: 'summary', visible: true, width: 'half', presentation: 'standard' },
				],
			}],
		}));
		const widgets = getActiveDashboardView(parsed).widgets;

		expect(widgets.slice(0, 9).map((widget) => widget.id)).toEqual([
			'alerts', 'service_health', 'container_health', 'network',
			'cpu_load', 'memory_usage', 'cpu_status', 'storage_summary', 'system',
		]);
		expect(widgets.filter((widget) => widget.id === 'service_health' || widget.id === 'container_health'))
			.toEqual([
				{ id: 'service_health', visible: false, width: 'quarter', presentation: 'standard', column: 0, row: 1, priority: 0 },
				{ id: 'container_health', visible: false, width: 'quarter', presentation: 'standard', column: 0, row: 1, priority: 0 },
			]);
		expect(widgets.filter((widget) => ['cpu_load', 'memory_usage', 'cpu_status', 'storage_summary'].includes(widget.id)).map((widget) => widget.width))
			.toEqual(['quarter', 'quarter', 'quarter', 'quarter']);
		expect(parsed.hiddenPresetTabs).toEqual(['overview']);
	});

	test('migrates v5 visible widgets without reserving space for hidden widgets', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			version: 5,
			preset: 'custom',
			hiddenPresetTabs: [],
			activeViewId: 'legacy',
			customViews: [{
				id: 'legacy',
				name: 'Legacy',
				density: 'comfortable',
				widgets: [
					{ id: 'alerts', visible: false, width: 'full', presentation: 'standard' },
					{ id: 'service_health', visible: true, width: 'half', presentation: 'standard' },
					{ id: 'container_health', visible: true, width: 'half', presentation: 'standard' },
				],
			}],
		}));
		const widgets = getActiveDashboardView(parsed).widgets;

		expect(widgets.find((widget) => widget.id === 'service_health')).toMatchObject({ column: 0, row: 0, priority: 0 });
		expect(widgets.find((widget) => widget.id === 'container_health')).toMatchObject({ column: 6, row: 0, priority: 0 });
	});

	test('normalizes named views, active selection, and newly added widget ids', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			version: 3,
			preset: 'custom',
			hiddenPresetTabs: ['storage', 'overview', 'storage', 'monitoring', 'unknown'],
			activeViewId: ' ops ',
			customViews: [
				{
					id: 'ops',
					name: 'Operations',
					density: 'compact',
					widgets: [
						{ id: 'storage', visible: false, width: 'half', presentation: 'tiny' },
						{ id: 'alerts', visible: true, width: 'quarter', presentation: 'tiny' },
						{ id: 'system', visible: true, width: 'third', presentation: 'tiny' },
						{ id: 'network', visible: true, width: 'quarter', presentation: 'standard' },
					],
				},
				{ id: 'ops', name: 'operations', widgets: [] },
			],
		}));

		expect(parsed.activeViewId).toBe('ops');
		expect(parsed.customViews.map((view) => view.id)).toEqual(['ops', 'custom-1']);
		expect(parsed.customViews.map((view) => view.name)).toEqual(['Operations', 'operations 2']);
		expect(new Set(parsed.customViews[0].widgets.map((widget) => widget.id)).size).toBe(13);
		expect(resolveDashboardWidgets(parsed)).not.toContainEqual(expect.objectContaining({ id: 'storage' }));
		expect(parsed.customViews[0].widgets.find((widget) => widget.id === 'storage')?.presentation).toBe('standard');
		expect(parsed.customViews[0].widgets.find((widget) => widget.id === 'alerts')?.presentation).toBe('tiny');
		expect(parsed.customViews[0].widgets.find((widget) => widget.id === 'alerts')?.width).toBe('quarter');
		expect(parsed.customViews[0].widgets.find((widget) => widget.id === 'system')?.width).toBe('third');
		expect(parsed.customViews[0].widgets.find((widget) => widget.id === 'network')?.width).toBe('half');
		expect(parsed.hiddenPresetTabs).toEqual(['overview', 'storage', 'monitoring']);
		expect(dashboardPresetTabVisible(parsed, 'overview')).toBe(false);
		expect(dashboardPresetTabVisible(parsed, 'storage')).toBe(false);
		expect(dashboardPresetTabVisible(parsed, 'monitoring')).toBe(false);
		expect(resolveDashboardDensity(parsed)).toBe('compact');
	});

	test('limits tiny presentation to widgets with a useful reduced form', () => {
		expect(dashboardWidgetIds.filter(dashboardWidgetSupportsTiny)).toEqual([
			'alerts', 'system', 'operations', 'history',
		]);
	});

	test('limits narrow widths to compact-safe presentations', () => {
		expect(dashboardWidgetSupportsNarrowWidth('alerts', 'standard')).toBe(false);
		expect(dashboardWidgetSupportsNarrowWidth('alerts', 'tiny')).toBe(true);
		for (const id of ['service_health', 'container_health', 'cpu_load', 'memory_usage', 'cpu_status', 'storage_summary'] as const) {
			expect(dashboardWidgetSupportsNarrowWidth(id, 'standard')).toBe(true);
		}
		expect(dashboardWidgetSupportsNarrowWidth('storage', 'standard')).toBe(false);
		expect(dashboardWidgetSupportsNarrowWidth('network', 'standard')).toBe(false);
		expect(dashboardWidgetSupportsNarrowWidth('disk_io', 'standard')).toBe(false);
		expect(dashboardWidgetSupportsNarrowWidth('operations', 'tiny')).toBe(true);
		expect(dashboardWidgetSupportsNarrowWidth('history', 'tiny')).toBe(true);
	});

	test('maps custom widths onto the 12-column dashboard grid', () => {
		expect(dashboardWidgetWidthClass('alerts', 'full')).toContain('xl:col-span-12');
		expect(dashboardWidgetWidthClass('alerts', 'half')).toContain('xl:col-span-6');
		expect(dashboardWidgetWidthClass('alerts', 'third')).toContain('xl:col-span-4');
		expect(dashboardWidgetWidthClass('alerts', 'quarter')).toContain('xl:col-span-3');
		expect(dashboardWidgetWidthClass('service_health', 'half')).toContain('md:col-span-6');
		expect(dashboardWidgetWidthClass('cpu_load', 'quarter')).toContain('lg:col-span-3');
	});

	test('hides any built-in preset tab and falls back to a custom view', () => {
		let preferences = createDashboardView(defaultDashboardPreferences(), 'Mine');
		const activeViewId = preferences.activeViewId;
		preferences.preset = 'overview';
		preferences = setDashboardPresetTabVisible(preferences, 'overview', false);

		expect(preferences.preset).toBe('custom');
		expect(preferences.activeViewId).toBe(activeViewId);
		expect(getActiveDashboardView(preferences).name).toBe('Mine');
		expect(dashboardPresetTabVisible(preferences, 'overview')).toBe(false);
		preferences = setDashboardPresetTabVisible(preferences, 'storage', false);
		preferences = setDashboardPresetTabVisible(preferences, 'monitoring', false);
		expect(dashboardPresetTabVisible(preferences, 'storage')).toBe(false);
		expect(dashboardPresetTabVisible(preferences, 'monitoring')).toBe(false);
		preferences = setDashboardPresetTabVisible(preferences, 'overview', true);
		expect(dashboardPresetTabVisible(preferences, 'overview')).toBe(true);
	});

	test('repairs a persisted active preset whose tab is hidden', () => {
		const parsed = parseDashboardPreferences(JSON.stringify({
			...defaultDashboardPreferences(),
			preset: 'storage',
			hiddenPresetTabs: ['storage'],
		}));

		expect(parsed.preset).toBe('custom');
		expect(parsed.hiddenPresetTabs).toEqual(['storage']);
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
		const independentTiles = ['service_health', 'container_health', 'cpu_load', 'memory_usage', 'cpu_status', 'storage_summary'];
		expect(resolveDashboardWidgets(preferences).map((widget) => widget.id)).toEqual(expect.arrayContaining(independentTiles));

		preferences.preset = 'storage';
		preferences.customViews[0].widgets = preferences.customViews[0].widgets.map((widget) => ({ ...widget, visible: false }));

		expect(resolveDashboardWidgets(preferences).map((widget) => widget.id)).toEqual([
			'alerts', 'system', 'operations', 'storage',
		]);
		expect(resolveDashboardDensity(preferences)).toBe('compact');
		preferences.preset = 'monitoring';
		expect(resolveDashboardWidgets(preferences).map((widget) => widget.id)).toEqual(expect.arrayContaining(independentTiles));
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

	test('persists the active named view in v6 storage', () => {
		const preferences = createDashboardView(defaultDashboardPreferences(), 'Monitoring desk');
		dashboardPrefs.set(preferences);

		expect(dashboardPrefs.value.preset).toBe('custom');
		expect(JSON.parse(localStorage.getItem(DASHBOARD_PREFERENCES_KEY) ?? '{}')).toMatchObject({
			version: 6,
			preset: 'custom',
			activeViewId: preferences.activeViewId,
		});
	});

	test('snaps widgets to starts that match their width', () => {
		expect(dashboardWidgetValidColumns('quarter')).toEqual([0, 3, 6, 9]);
		expect(dashboardWidgetValidColumns('third')).toEqual([0, 4, 8]);
		expect(dashboardWidgetValidColumns('half')).toEqual([0, 6]);
		expect(dashboardWidgetValidColumns('full')).toEqual([0]);
		expect(dashboardWidgetSnapColumn('quarter', 8)).toBe(9);
		expect(dashboardWidgetSnapColumn('half', 4)).toBe(6);
	});

	test('packs dynamic widget heights without overlapping neighboring columns', () => {
		const widgets: DashboardWidgetConfig[] = [
			{ id: 'storage', visible: true, width: 'half', presentation: 'standard', column: 0, row: 0, priority: 0 },
			{ id: 'cpu_load', visible: true, width: 'quarter', presentation: 'standard', column: 6, row: 0, priority: 0 },
			{ id: 'memory_usage', visible: true, width: 'quarter', presentation: 'standard', column: 9, row: 0, priority: 0 },
			{ id: 'cpu_status', visible: true, width: 'quarter', presentation: 'standard', column: 6, row: 1, priority: 0 },
		];
		const layout = layoutDashboardWidgets(widgets, { storage: 8, cpu_load: 2, memory_usage: 2, cpu_status: 2 });

		expect(layout.storage).toMatchObject({ column: 0, row: 0, columnSpan: 6, rowSpan: 8 });
		expect(layout.cpu_load).toMatchObject({ column: 6, row: 0, rowSpan: 2 });
		expect(layout.memory_usage).toMatchObject({ column: 9, row: 0, rowSpan: 2 });
		expect(layout.cpu_status).toMatchObject({ column: 6, row: 2, rowSpan: 2 });
	});

	test('places a moved widget first and pushes collisions down without mutating the view', () => {
		const widgets: DashboardWidgetConfig[] = [
			{ id: 'storage', visible: true, width: 'half', presentation: 'standard', column: 0, row: 0, priority: 0 },
			{ id: 'cpu_load', visible: true, width: 'quarter', presentation: 'standard', column: 6, row: 0, priority: 0 },
			{ id: 'cpu_status', visible: true, width: 'quarter', presentation: 'standard', column: 6, row: 2, priority: 0 },
		];
		const placed = placeDashboardWidget(widgets, 'cpu_status', 0, 0);

		expect(placed.find((widget) => widget.id === 'cpu_status')).toMatchObject({ column: 0, row: 0 });
		expect(placed.find((widget) => widget.id === 'cpu_status')?.priority).toBe(1);
		expect(placed.find((widget) => widget.id === 'storage')).toMatchObject({ column: 0, row: 0, priority: 0 });
		expect(placed.find((widget) => widget.id === 'cpu_load')).toMatchObject({ column: 6, row: 0 });
		expect(widgets.find((widget) => widget.id === 'storage')).toMatchObject({ column: 0, row: 0 });

		const layout = layoutDashboardWidgets(placed, { storage: 8, cpu_load: 2, cpu_status: 2 });
		expect(layout.cpu_status).toMatchObject({ column: 0, row: 0 });
		expect(layout.storage).toMatchObject({ column: 0, row: 2 });

		const shorterLayout = layoutDashboardWidgets(placed, { storage: 8, cpu_load: 2, cpu_status: 1 });
		expect(shorterLayout.storage).toMatchObject({ column: 0, row: 1 });
	});
});
