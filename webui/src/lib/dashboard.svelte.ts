export const DASHBOARD_PREFERENCES_KEY = 'nasty:dashboard:v4';
export const LEGACY_DASHBOARD_V3_PREFERENCES_KEY = 'nasty:dashboard:v3';
export const LEGACY_DASHBOARD_V2_PREFERENCES_KEY = 'nasty:dashboard:v2';
export const LEGACY_DASHBOARD_PREFERENCES_KEY = 'nasty:dashboard:v1';
export const DASHBOARD_PREFERENCES_VERSION = 4;
export const DASHBOARD_VIEW_NAME_MAX_LENGTH = 40;

export const dashboardWidgetIds = [
	'alerts',
	'system',
	'health',
	'summary',
	'operations',
	'storage',
	'history',
	'network',
	'disk_io',
] as const;

export type DashboardWidgetId = (typeof dashboardWidgetIds)[number];
export type DashboardPreset = 'overview' | 'storage' | 'monitoring' | 'custom';
export type DashboardFixedPreset = Exclude<DashboardPreset, 'custom'>;
export type DashboardOptionalPreset = Exclude<DashboardFixedPreset, 'overview'>;
export type DashboardDensity = 'comfortable' | 'compact';
export type DashboardWidgetWidth = 'quarter' | 'third' | 'half' | 'full';
export type DashboardWidgetPresentation = 'standard' | 'tiny';

export const dashboardOptionalPresets: DashboardOptionalPreset[] = ['storage', 'monitoring'];

export interface DashboardWidgetConfig {
	id: DashboardWidgetId;
	visible: boolean;
	width: DashboardWidgetWidth;
	presentation: DashboardWidgetPresentation;
}

export interface DashboardCustomView {
	id: string;
	name: string;
	density: DashboardDensity;
	widgets: DashboardWidgetConfig[];
}

export interface DashboardPreferences {
	version: typeof DASHBOARD_PREFERENCES_VERSION;
	preset: DashboardPreset;
	hiddenPresetTabs: DashboardOptionalPreset[];
	activeViewId: string;
	customViews: DashboardCustomView[];
}

export const dashboardWidgetMeta: Record<DashboardWidgetId, { label: string; description: string; supportsTiny: boolean }> = {
	alerts: { label: 'Alerts', description: 'Active warnings and critical conditions.', supportsTiny: true },
	system: { label: 'System status', description: 'Host identity, uptime, and service health.', supportsTiny: true },
	health: { label: 'Service and container health', description: 'Enabled services and expected managed containers currently running.', supportsTiny: false },
	summary: { label: 'Resource summary', description: 'CPU, memory, temperature, and total storage.', supportsTiny: false },
	operations: { label: 'Active operations', description: 'Scrubs, evacuations, and reconciliation work.', supportsTiny: true },
	storage: { label: 'Compact storage', description: 'Filesystems and member devices in a dense table.', supportsTiny: false },
	history: { label: 'CPU and memory history', description: 'Resource history for the selected time range.', supportsTiny: true },
	network: { label: 'Network', description: 'Interface status, throughput, and traffic history.', supportsTiny: false },
	disk_io: { label: 'Disk I/O', description: 'Per-device read and write activity.', supportsTiny: false },
};

export function dashboardWidgetSupportsTiny(id: DashboardWidgetId): boolean {
	return dashboardWidgetMeta[id].supportsTiny;
}

export function dashboardWidgetSupportsNarrowWidth(
	id: DashboardWidgetId,
	presentation: DashboardWidgetPresentation,
): boolean {
	return id === 'health' || id === 'summary' || (dashboardWidgetSupportsTiny(id) && presentation === 'tiny');
}

export function dashboardWidgetWidthClass(width: DashboardWidgetWidth): string {
	if (width === 'quarter') return 'min-w-0 xl:col-span-3';
	if (width === 'third') return 'min-w-0 xl:col-span-4';
	if (width === 'half') return 'min-w-0 xl:col-span-6';
	return 'min-w-0 xl:col-span-12';
}

const defaultCustomWidgets: DashboardWidgetConfig[] = [
	{ id: 'alerts', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'system', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'health', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'summary', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'operations', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'storage', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'history', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'network', visible: true, width: 'half', presentation: 'standard' },
	{ id: 'disk_io', visible: true, width: 'half', presentation: 'standard' },
];

export const dashboardPresets: Record<DashboardFixedPreset, {
	label: string;
	description: string;
	density: DashboardDensity;
	widgets: DashboardWidgetConfig[];
}> = {
	overview: {
		label: 'Overview',
		description: 'The balanced system and performance dashboard.',
		density: 'comfortable',
		widgets: defaultCustomWidgets.filter((widget) => widget.id !== 'storage' && widget.id !== 'operations'),
	},
	storage: {
		label: 'Storage Compact',
		description: 'A dense one-page view of filesystems and member disks.',
		density: 'compact',
		widgets: defaultCustomWidgets.filter((widget) =>
			['alerts', 'system', 'operations', 'storage'].includes(widget.id)
		),
	},
	monitoring: {
		label: 'Monitoring',
		description: 'Resource history and live network and disk activity.',
		density: 'comfortable',
		widgets: defaultCustomWidgets.filter((widget) => widget.id !== 'storage' && widget.id !== 'system'),
	},
};

function cloneWidgets(widgets: DashboardWidgetConfig[]): DashboardWidgetConfig[] {
	return widgets.map((widget) => ({ ...widget }));
}

function defaultCustomView(id = 'custom-1', name = 'Custom'): DashboardCustomView {
	return {
		id,
		name,
		density: 'comfortable',
		widgets: cloneWidgets(defaultCustomWidgets),
	};
}

export function defaultDashboardPreferences(): DashboardPreferences {
	const customView = defaultCustomView();
	return {
		version: DASHBOARD_PREFERENCES_VERSION,
		preset: 'overview',
		hiddenPresetTabs: [],
		activeViewId: customView.id,
		customViews: [customView],
	};
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizeWidgets(value: unknown): DashboardWidgetConfig[] {
	const configured = Array.isArray(value) ? value : [];
	const byId = new Map<DashboardWidgetId, DashboardWidgetConfig>();
	for (const candidate of configured) {
		if (!isRecord(candidate) || !dashboardWidgetIds.includes(candidate.id as DashboardWidgetId)) continue;
		const id = candidate.id as DashboardWidgetId;
		if (byId.has(id)) continue;
		const presentation = candidate.presentation === 'tiny' && dashboardWidgetSupportsTiny(id) ? 'tiny' : 'standard';
		const requestedWidth = candidate.width === 'quarter' || candidate.width === 'third' || candidate.width === 'half'
			? candidate.width
			: 'full';
		byId.set(id, {
			id,
			visible: candidate.visible !== false,
			width: (requestedWidth === 'quarter' || requestedWidth === 'third') && !dashboardWidgetSupportsNarrowWidth(id, presentation)
				? 'half'
				: requestedWidth,
			presentation,
		});
	}

	const widgets = [...byId.values()];
	for (const widget of defaultCustomWidgets) {
		if (!byId.has(widget.id)) widgets.push({ ...widget });
	}
	if (!widgets.some((widget) => widget.visible)) widgets[0] = { ...widgets[0], visible: true };
	return widgets;
}

function normalizePreset(value: unknown): DashboardPreset {
	return value === 'storage' || value === 'monitoring' || value === 'custom' ? value : 'overview';
}

function normalizeHiddenPresetTabs(value: unknown): DashboardOptionalPreset[] {
	if (!Array.isArray(value)) return [];
	return dashboardOptionalPresets.filter((preset) => value.includes(preset));
}

function normalizeViewName(value: unknown, fallback: string): string {
	if (typeof value !== 'string') return fallback;
	return value.trim().slice(0, DASHBOARD_VIEW_NAME_MAX_LENGTH) || fallback;
}

function nextViewId(ids: Iterable<string>): string {
	const used = new Set(ids);
	let index = 1;
	while (used.has(`custom-${index}`)) index += 1;
	return `custom-${index}`;
}

function uniqueViewName(name: string, names: Iterable<string>): string {
	const used = new Set([...names].map((candidate) => candidate.toLocaleLowerCase()));
	const base = normalizeViewName(name, 'Custom');
	if (!used.has(base.toLocaleLowerCase())) return base;
	let index = 2;
	let candidate = '';
	do {
		const suffix = ` ${index}`;
		candidate = `${base.slice(0, DASHBOARD_VIEW_NAME_MAX_LENGTH - suffix.length)}${suffix}`;
		index += 1;
	} while (used.has(candidate.toLocaleLowerCase()));
	return candidate;
}

function normalizeCustomViews(value: unknown): DashboardCustomView[] {
	const configured = Array.isArray(value) ? value : [];
	const views: DashboardCustomView[] = [];
	for (const candidate of configured) {
		if (!isRecord(candidate)) continue;
		const requestedId = typeof candidate.id === 'string' ? candidate.id.trim() : '';
		const id = requestedId && !views.some((view) => view.id === requestedId)
			? requestedId
			: nextViewId(views.map((view) => view.id));
		const fallbackName = views.length === 0 ? 'Custom' : `Custom ${views.length + 1}`;
		const name = uniqueViewName(
			normalizeViewName(candidate.name, fallbackName),
			views.map((view) => view.name),
		);
		views.push({
			id,
			name,
			density: candidate.density === 'compact' ? 'compact' : 'comfortable',
			widgets: normalizeWidgets(candidate.widgets),
		});
	}
	return views.length > 0 ? views : [defaultCustomView()];
}

function migrateLegacyPreferences(value: Record<string, unknown>): DashboardPreferences {
	const customView: DashboardCustomView = {
		...defaultCustomView(),
		density: value.density === 'compact' ? 'compact' : 'comfortable',
		widgets: normalizeWidgets(value.widgets),
	};
	return {
		version: DASHBOARD_PREFERENCES_VERSION,
		preset: normalizePreset(value.preset),
		hiddenPresetTabs: [],
		activeViewId: customView.id,
		customViews: [customView],
	};
}

function normalizeDashboardPreferences(value: unknown): DashboardPreferences {
	if (!isRecord(value)) return defaultDashboardPreferences();
	if (value.version === 1) return migrateLegacyPreferences(value);
	if (value.version !== 2 && value.version !== 3 && value.version !== DASHBOARD_PREFERENCES_VERSION) return defaultDashboardPreferences();

	const customViews = normalizeCustomViews(value.customViews);
	const requestedActiveViewId = typeof value.activeViewId === 'string' ? value.activeViewId.trim() : '';
	const activeViewId = customViews.some((view) => view.id === requestedActiveViewId)
		? requestedActiveViewId
		: customViews[0].id;

	const hiddenPresetTabs = normalizeHiddenPresetTabs(value.hiddenPresetTabs);
	const requestedPreset = normalizePreset(value.preset);
	return {
		version: DASHBOARD_PREFERENCES_VERSION,
		preset: requestedPreset !== 'custom' && requestedPreset !== 'overview' && hiddenPresetTabs.includes(requestedPreset)
			? 'overview'
			: requestedPreset,
		hiddenPresetTabs,
		activeViewId,
		customViews,
	};
}

export function parseDashboardPreferences(raw: string | null): DashboardPreferences {
	if (!raw) return defaultDashboardPreferences();
	try {
		return normalizeDashboardPreferences(JSON.parse(raw));
	} catch {
		return defaultDashboardPreferences();
	}
}

export function loadDashboardPreferences(storage: Pick<Storage, 'getItem'>): DashboardPreferences {
	return parseDashboardPreferences(
		storage.getItem(DASHBOARD_PREFERENCES_KEY)
		?? storage.getItem(LEGACY_DASHBOARD_V3_PREFERENCES_KEY)
		?? storage.getItem(LEGACY_DASHBOARD_V2_PREFERENCES_KEY)
		?? storage.getItem(LEGACY_DASHBOARD_PREFERENCES_KEY)
	);
}

export function initializeDashboardPreferences(storage: Pick<Storage, 'getItem' | 'setItem'>): DashboardPreferences {
	const preferences = loadDashboardPreferences(storage);
	if (!storage.getItem(DASHBOARD_PREFERENCES_KEY)) {
		storage.setItem(DASHBOARD_PREFERENCES_KEY, JSON.stringify(preferences));
	}
	return preferences;
}

export function getActiveDashboardView(preferences: DashboardPreferences): DashboardCustomView {
	return preferences.customViews.find((view) => view.id === preferences.activeViewId)
		?? preferences.customViews[0]
		?? defaultCustomView();
}

export function resolveDashboardWidgets(preferences: DashboardPreferences): DashboardWidgetConfig[] {
	const widgets = preferences.preset === 'custom'
		? getActiveDashboardView(preferences).widgets
		: dashboardPresets[preferences.preset].widgets;
	return cloneWidgets(widgets).filter((widget) => widget.visible);
}

export function resolveDashboardDensity(preferences: DashboardPreferences): DashboardDensity {
	return preferences.preset === 'custom'
		? getActiveDashboardView(preferences).density
		: dashboardPresets[preferences.preset].density;
}

export function dashboardPresetTabVisible(
	preferences: DashboardPreferences,
	preset: DashboardFixedPreset,
): boolean {
	return preset === 'overview' || !preferences.hiddenPresetTabs.includes(preset);
}

export function setDashboardPresetTabVisible(
	preferences: DashboardPreferences,
	preset: DashboardOptionalPreset,
	visible: boolean,
): DashboardPreferences {
	const hiddenPresetTabs = visible
		? preferences.hiddenPresetTabs.filter((candidate) => candidate !== preset)
		: [...new Set([...preferences.hiddenPresetTabs, preset])];
	return {
		...preferences,
		hiddenPresetTabs,
		preset: !visible && preferences.preset === preset ? 'overview' : preferences.preset,
	};
}

export function dashboardViewNameAvailable(
	preferences: DashboardPreferences,
	name: string,
	excludeId?: string,
): boolean {
	const normalized = name.trim().slice(0, DASHBOARD_VIEW_NAME_MAX_LENGTH).toLocaleLowerCase();
	return normalized.length > 0 && !preferences.customViews.some((view) =>
		view.id !== excludeId && view.name.toLocaleLowerCase() === normalized
	);
}

export function createDashboardView(
	preferences: DashboardPreferences,
	name: string,
	source?: DashboardCustomView,
): DashboardPreferences {
	const id = nextViewId(preferences.customViews.map((view) => view.id));
	const base = source ?? defaultCustomView(id);
	const view: DashboardCustomView = {
		id,
		name: uniqueViewName(name, preferences.customViews.map((candidate) => candidate.name)),
		density: base.density,
		widgets: cloneWidgets(base.widgets),
	};
	return {
		...preferences,
		preset: 'custom',
		activeViewId: id,
		customViews: [...preferences.customViews, view],
	};
}

export function renameDashboardView(
	preferences: DashboardPreferences,
	id: string,
	name: string,
): DashboardPreferences {
	if (!dashboardViewNameAvailable(preferences, name, id)) return preferences;
	return {
		...preferences,
		customViews: preferences.customViews.map((view) =>
			view.id === id ? { ...view, name: normalizeViewName(name, view.name) } : view
		),
	};
}

export function deleteDashboardView(preferences: DashboardPreferences, id: string): DashboardPreferences {
	if (preferences.customViews.length <= 1) return preferences;
	const index = preferences.customViews.findIndex((view) => view.id === id);
	if (index < 0) return preferences;
	const customViews = preferences.customViews.filter((view) => view.id !== id);
	const activeViewId = preferences.activeViewId === id
		? customViews[Math.min(index, customViews.length - 1)].id
		: preferences.activeViewId;
	return { ...preferences, activeViewId, customViews };
}

export function selectDashboardView(preferences: DashboardPreferences, id: string): DashboardPreferences {
	if (!preferences.customViews.some((view) => view.id === id)) return preferences;
	return { ...preferences, preset: 'custom', activeViewId: id };
}

export function updateActiveDashboardView(
	preferences: DashboardPreferences,
	patch: Partial<Pick<DashboardCustomView, 'density' | 'widgets'>>,
): DashboardPreferences {
	return {
		...preferences,
		customViews: preferences.customViews.map((view) =>
			view.id === preferences.activeViewId
				? { ...view, ...patch, widgets: patch.widgets ? cloneWidgets(patch.widgets) : view.widgets }
				: view
		),
	};
}

export function swapDashboardWidgets(
	widgets: DashboardWidgetConfig[],
	source: DashboardWidgetId,
	target: DashboardWidgetId,
): DashboardWidgetConfig[] {
	const sourceIndex = widgets.findIndex((widget) => widget.id === source);
	const targetIndex = widgets.findIndex((widget) => widget.id === target);
	if (sourceIndex < 0 || targetIndex < 0 || sourceIndex === targetIndex) return cloneWidgets(widgets);

	const next = cloneWidgets(widgets);
	[next[sourceIndex], next[targetIndex]] = [next[targetIndex], next[sourceIndex]];
	return next;
}

function createDashboardPrefs() {
	const storage = typeof localStorage !== 'undefined' ? localStorage : null;
	const initialValue = storage ? initializeDashboardPreferences(storage) : defaultDashboardPreferences();
	let value = $state<DashboardPreferences>(initialValue);

	function set(next: DashboardPreferences) {
		value = normalizeDashboardPreferences(next);
		storage?.setItem(DASHBOARD_PREFERENCES_KEY, JSON.stringify(value));
	}

	return {
		get value() {
			return value;
		},
		set,
		reset() {
			set(defaultDashboardPreferences());
		},
	};
}

export const dashboardPrefs = createDashboardPrefs();
