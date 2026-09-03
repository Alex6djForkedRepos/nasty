export const DASHBOARD_PREFERENCES_KEY = 'nasty:dashboard:v7';
export const LEGACY_DASHBOARD_V6_PREFERENCES_KEY = 'nasty:dashboard:v6';
export const LEGACY_DASHBOARD_V5_PREFERENCES_KEY = 'nasty:dashboard:v5';
export const LEGACY_DASHBOARD_V4_PREFERENCES_KEY = 'nasty:dashboard:v4';
export const LEGACY_DASHBOARD_V3_PREFERENCES_KEY = 'nasty:dashboard:v3';
export const LEGACY_DASHBOARD_V2_PREFERENCES_KEY = 'nasty:dashboard:v2';
export const LEGACY_DASHBOARD_PREFERENCES_KEY = 'nasty:dashboard:v1';
export const DASHBOARD_PREFERENCES_VERSION = 7;
export const DASHBOARD_VIEW_NAME_MAX_LENGTH = 40;
export const DASHBOARD_GRID_COLUMNS = 12;

export const dashboardWidgetIds = [
	'alerts',
	'system',
	'service_health',
	'container_health',
	'compute',
	'clock',
	'schedule',
	'cpu_load',
	'memory_usage',
	'cpu_status',
	'storage_summary',
	'operations',
	'storage',
	'history',
	'network',
	'disk_io',
] as const;

export type DashboardWidgetId = (typeof dashboardWidgetIds)[number];
export type DashboardPreset = 'overview' | 'storage' | 'monitoring' | 'custom';
export type DashboardFixedPreset = Exclude<DashboardPreset, 'custom'>;
export type DashboardOptionalPreset = DashboardFixedPreset;
export type DashboardDensity = 'comfortable' | 'compact';
export type DashboardWidgetWidth = 'quarter' | 'third' | 'half' | 'full';
export type DashboardWidgetPresentation = 'standard' | 'tiny';

export const dashboardOptionalPresets: DashboardOptionalPreset[] = ['overview', 'storage', 'monitoring'];

export interface DashboardWidgetConfig {
	id: DashboardWidgetId;
	visible: boolean;
	width: DashboardWidgetWidth;
	presentation: DashboardWidgetPresentation;
	column: number;
	row: number;
	priority: number;
}

export interface DashboardGridPosition {
	column: number;
	row: number;
	columnSpan: number;
	rowSpan: number;
}

export type DashboardGridLayout = Partial<Record<DashboardWidgetId, DashboardGridPosition>>;
export type DashboardWidgetRowSpans = Partial<Record<DashboardWidgetId, number>>;

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
	service_health: { label: 'Service health', description: 'Enabled services currently running.', supportsTiny: false },
	container_health: { label: 'Container health', description: 'Expected managed containers currently running.', supportsTiny: false },
	compute: { label: 'Compute', description: 'Virtual machines and Docker workloads at a glance.', supportsTiny: false },
	clock: { label: 'Clock & notice', description: 'Host time, date, timezone, and dashboard notice.', supportsTiny: false },
	schedule: { label: 'Backup schedule', description: 'Upcoming scheduled backup runs.', supportsTiny: false },
	cpu_load: { label: 'CPU load', description: 'Current load across available CPU cores.', supportsTiny: false },
	memory_usage: { label: 'Memory', description: 'Current memory use and bcachefs cache.', supportsTiny: false },
	cpu_status: { label: 'CPU status', description: 'CPU temperature, frequency, and governor.', supportsTiny: false },
	storage_summary: { label: 'Storage summary', description: 'Combined mounted filesystem capacity.', supportsTiny: false },
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
	return ['service_health', 'container_health', 'compute', 'clock', 'cpu_load', 'memory_usage', 'cpu_status', 'storage_summary'].includes(id)
		|| (dashboardWidgetSupportsTiny(id) && presentation === 'tiny');
}

export function dashboardWidgetColumnSpan(width: DashboardWidgetWidth): number {
	if (width === 'quarter') return 3;
	if (width === 'third') return 4;
	if (width === 'half') return 6;
	return DASHBOARD_GRID_COLUMNS;
}

export function dashboardWidgetValidColumns(width: DashboardWidgetWidth): number[] {
	const span = dashboardWidgetColumnSpan(width);
	return Array.from({ length: DASHBOARD_GRID_COLUMNS / span }, (_, index) => index * span);
}

export function dashboardWidgetSnapColumn(width: DashboardWidgetWidth, requested: number): number {
	const columns = dashboardWidgetValidColumns(width);
	if (!Number.isFinite(requested)) return columns[0];
	return columns.reduce((nearest, candidate) =>
		Math.abs(candidate - requested) < Math.abs(nearest - requested) ? candidate : nearest
	);
}

export function dashboardWidgetWidthClass(id: DashboardWidgetId, width: DashboardWidgetWidth): string {
	const responsive = id === 'service_health' || id === 'container_health' || id === 'compute' || id === 'schedule'
		? 'col-span-12 md:col-span-6'
		: id === 'clock'
			? 'col-span-12 md:col-span-6'
			: ['cpu_load', 'memory_usage', 'cpu_status', 'storage_summary'].includes(id)
			? 'col-span-6 lg:col-span-3'
			: 'col-span-12';
	if (width === 'quarter') return `min-w-0 ${responsive} xl:col-span-3`;
	if (width === 'third') return `min-w-0 ${responsive} xl:col-span-4`;
	if (width === 'half') return `min-w-0 ${responsive} xl:col-span-6`;
	return `min-w-0 ${responsive} xl:col-span-12`;
}

export function updateDashboardWidgetAppearance(
	widgets: DashboardWidgetConfig[],
	id: DashboardWidgetId,
	patch: Partial<Pick<DashboardWidgetConfig, 'width' | 'presentation'>>,
): DashboardWidgetConfig[] {
	return widgets.map((widget) => {
		if (widget.id !== id) return widget;
		const presentation = patch.presentation === 'tiny' && dashboardWidgetSupportsTiny(id)
			? 'tiny'
			: patch.presentation === 'standard' ? 'standard' : widget.presentation;
		const requestedWidth = patch.width ?? widget.width;
		const width = (requestedWidth === 'quarter' || requestedWidth === 'third')
			&& !dashboardWidgetSupportsNarrowWidth(id, presentation)
			? 'half'
			: requestedWidth;
		return {
			...widget,
			width,
			presentation,
			column: dashboardWidgetSnapColumn(width, widget.column),
		};
	});
}

type DashboardWidgetDefinition = Omit<DashboardWidgetConfig, 'column' | 'row' | 'priority'>;

function positionWidgets(widgets: DashboardWidgetDefinition[]): DashboardWidgetConfig[] {
	let column = 0;
	let row = 0;
	return widgets.map((widget) => {
		const columnSpan = dashboardWidgetColumnSpan(widget.width);
		if (column + columnSpan > DASHBOARD_GRID_COLUMNS) {
			column = 0;
			row += 1;
		}
		const positioned = { ...widget, column, row, priority: 0 };
		column += columnSpan;
		if (column === DASHBOARD_GRID_COLUMNS) {
			column = 0;
			row += 1;
		}
		return positioned;
	});
}

const defaultCustomWidgets: DashboardWidgetConfig[] = positionWidgets([
	{ id: 'alerts', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'system', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'service_health', visible: true, width: 'half', presentation: 'standard' },
	{ id: 'container_health', visible: true, width: 'half', presentation: 'standard' },
	{ id: 'cpu_load', visible: true, width: 'quarter', presentation: 'standard' },
	{ id: 'memory_usage', visible: true, width: 'quarter', presentation: 'standard' },
	{ id: 'cpu_status', visible: true, width: 'quarter', presentation: 'standard' },
	{ id: 'storage_summary', visible: true, width: 'quarter', presentation: 'standard' },
	{ id: 'operations', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'storage', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'history', visible: true, width: 'full', presentation: 'standard' },
	{ id: 'network', visible: true, width: 'half', presentation: 'standard' },
	{ id: 'disk_io', visible: true, width: 'half', presentation: 'standard' },
	{ id: 'compute', visible: false, width: 'half', presentation: 'standard' },
	{ id: 'clock', visible: false, width: 'quarter', presentation: 'standard' },
	{ id: 'schedule', visible: false, width: 'half', presentation: 'standard' },
]);

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

function expandLegacyWidgets(configured: unknown[]): unknown[] {
	return configured.flatMap((candidate) => {
		if (!isRecord(candidate) || (candidate.id !== 'health' && candidate.id !== 'summary')) return [candidate];
		const requestedWidth: DashboardWidgetWidth = candidate.width === 'quarter' || candidate.width === 'third' || candidate.width === 'half'
			? candidate.width
			: 'full';
		const width: DashboardWidgetWidth = candidate.id === 'health'
			? requestedWidth === 'full' ? 'half' : requestedWidth === 'half' ? 'quarter' : requestedWidth
			: requestedWidth === 'full' || requestedWidth === 'half' ? 'quarter' : requestedWidth;
		const ids: DashboardWidgetId[] = candidate.id === 'health'
			? ['service_health', 'container_health']
			: ['cpu_load', 'memory_usage', 'cpu_status', 'storage_summary'];
		return ids.map((id) => ({ ...candidate, id, width, presentation: 'standard' }));
	});
}

function normalizeWidgets(value: unknown): DashboardWidgetConfig[] {
	const configured = expandLegacyWidgets(Array.isArray(value) ? value : []);
	const byId = new Map<DashboardWidgetId, DashboardWidgetConfig>();
	let fallbackColumn = 0;
	let fallbackRow = 0;
	const nextFallbackPosition = (width: DashboardWidgetWidth) => {
		const columnSpan = dashboardWidgetColumnSpan(width);
		if (fallbackColumn + columnSpan > DASHBOARD_GRID_COLUMNS) {
			fallbackColumn = 0;
			fallbackRow += 1;
		}
		const position = { column: fallbackColumn, row: fallbackRow };
		fallbackColumn += columnSpan;
		if (fallbackColumn === DASHBOARD_GRID_COLUMNS) {
			fallbackColumn = 0;
			fallbackRow += 1;
		}
		return position;
	};
	for (const candidate of configured) {
		if (!isRecord(candidate) || !dashboardWidgetIds.includes(candidate.id as DashboardWidgetId)) continue;
		const id = candidate.id as DashboardWidgetId;
		if (byId.has(id)) continue;
		const presentation = candidate.presentation === 'tiny' && dashboardWidgetSupportsTiny(id) ? 'tiny' : 'standard';
		const requestedWidth = candidate.width === 'quarter' || candidate.width === 'third' || candidate.width === 'half'
			? candidate.width
			: 'full';
		const width = (requestedWidth === 'quarter' || requestedWidth === 'third') && !dashboardWidgetSupportsNarrowWidth(id, presentation)
			? 'half'
			: requestedWidth;
		const visible = candidate.visible !== false;
		const fallback = visible ? nextFallbackPosition(width) : { column: fallbackColumn, row: fallbackRow };
		byId.set(id, {
			id,
			visible,
			width,
			presentation,
			column: typeof candidate.column === 'number' && Number.isInteger(candidate.column)
				? dashboardWidgetSnapColumn(width, candidate.column)
				: fallback.column,
			row: typeof candidate.row === 'number' && Number.isInteger(candidate.row)
				? Math.max(0, Math.min(10_000, candidate.row))
				: fallback.row,
			priority: typeof candidate.priority === 'number' && Number.isInteger(candidate.priority)
				? Math.max(0, Math.min(1_000_000_000, candidate.priority))
				: 0,
		});
	}

	const widgets = [...byId.values()];
	for (const widget of defaultCustomWidgets) {
		if (!byId.has(widget.id)) widgets.push({ ...widget, ...nextFallbackPosition(widget.width), priority: 0 });
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
	if (value.version !== 2 && value.version !== 3 && value.version !== 4 && value.version !== 5 && value.version !== 6 && value.version !== DASHBOARD_PREFERENCES_VERSION) return defaultDashboardPreferences();

	const customViews = normalizeCustomViews(value.customViews);
	const requestedActiveViewId = typeof value.activeViewId === 'string' ? value.activeViewId.trim() : '';
	const activeViewId = customViews.some((view) => view.id === requestedActiveViewId)
		? requestedActiveViewId
		: customViews[0].id;

	const hiddenPresetTabs = normalizeHiddenPresetTabs(value.hiddenPresetTabs);
	const requestedPreset = normalizePreset(value.preset);
	return {
		version: DASHBOARD_PREFERENCES_VERSION,
		preset: requestedPreset !== 'custom' && hiddenPresetTabs.includes(requestedPreset)
			? 'custom'
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
		?? storage.getItem(LEGACY_DASHBOARD_V6_PREFERENCES_KEY)
		?? storage.getItem(LEGACY_DASHBOARD_V5_PREFERENCES_KEY)
		?? storage.getItem(LEGACY_DASHBOARD_V4_PREFERENCES_KEY)
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
	return !preferences.hiddenPresetTabs.includes(preset);
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
		preset: !visible && preferences.preset === preset ? 'custom' : preferences.preset,
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

function rectanglesOverlap(a: DashboardGridPosition, b: DashboardGridPosition): boolean {
	return a.column < b.column + b.columnSpan
		&& a.column + a.columnSpan > b.column
		&& a.row < b.row + b.rowSpan
		&& a.row + a.rowSpan > b.row;
}

export function layoutDashboardWidgets(
	widgets: DashboardWidgetConfig[],
	rowSpans: DashboardWidgetRowSpans,
	moving?: { id: DashboardWidgetId; column: number; row: number },
): DashboardGridLayout {
	const movingPriority = Math.max(0, ...widgets.map((widget) => widget.priority)) + 1;
	const candidates = widgets
		.filter((widget) => widget.visible)
		.map((widget, index) => ({
			id: widget.id,
			column: moving?.id === widget.id
				? dashboardWidgetSnapColumn(widget.width, moving.column)
				: dashboardWidgetSnapColumn(widget.width, widget.column),
			row: moving?.id === widget.id ? Math.max(0, Math.round(moving.row)) : widget.row,
			columnSpan: dashboardWidgetColumnSpan(widget.width),
			rowSpan: Math.max(1, Math.round(rowSpans[widget.id] ?? 1)),
			priority: moving?.id === widget.id ? movingPriority : widget.priority,
			index,
		}));
	candidates.sort((a, b) => a.row - b.row || b.priority - a.priority || a.column - b.column || a.index - b.index);

	const placed: DashboardGridPosition[] = [];
	const layout: DashboardGridLayout = {};
	for (const candidate of candidates) {
		const position: DashboardGridPosition = { ...candidate };
		let collisions = placed.filter((existing) => rectanglesOverlap(position, existing));
		while (collisions.length > 0) {
			position.row = Math.max(...collisions.map((existing) => existing.row + existing.rowSpan));
			collisions = placed.filter((existing) => rectanglesOverlap(position, existing));
		}
		placed.push(position);
		layout[candidate.id] = position;
	}
	return layout;
}

export function placeDashboardWidget(
	widgets: DashboardWidgetConfig[],
	id: DashboardWidgetId,
	column: number,
	row: number,
): DashboardWidgetConfig[] {
	if (!widgets.some((widget) => widget.id === id && widget.visible)) return cloneWidgets(widgets);
	const widget = widgets.find((candidate) => candidate.id === id)!;
	const nextPriority = Math.max(0, ...widgets.map((candidate) => candidate.priority)) + 1;
	return cloneWidgets(widgets)
		.map((candidate) => candidate.id === id
			? {
				...candidate,
				column: dashboardWidgetSnapColumn(widget.width, column),
				row: Math.max(0, Math.round(row)),
				priority: nextPriority,
			}
			: candidate
		)
		.sort((a, b) => a.row - b.row || a.column - b.column || dashboardWidgetIds.indexOf(a.id) - dashboardWidgetIds.indexOf(b.id));
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
