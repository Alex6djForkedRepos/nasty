export const DASHBOARD_PREFERENCES_KEY = 'nasty:dashboard:v1';
export const DASHBOARD_PREFERENCES_VERSION = 1;

export const dashboardWidgetIds = [
	'alerts',
	'system',
	'summary',
	'operations',
	'storage',
	'history',
	'network',
	'disk_io',
] as const;

export type DashboardWidgetId = (typeof dashboardWidgetIds)[number];
export type DashboardPreset = 'overview' | 'storage' | 'monitoring' | 'custom';
export type DashboardDensity = 'comfortable' | 'compact';
export type DashboardWidgetWidth = 'half' | 'full';

export interface DashboardWidgetConfig {
	id: DashboardWidgetId;
	visible: boolean;
	width: DashboardWidgetWidth;
}

export interface DashboardPreferences {
	version: typeof DASHBOARD_PREFERENCES_VERSION;
	preset: DashboardPreset;
	density: DashboardDensity;
	widgets: DashboardWidgetConfig[];
}

export const dashboardWidgetMeta: Record<DashboardWidgetId, { label: string; description: string }> = {
	alerts: { label: 'Alerts', description: 'Active warnings and critical conditions.' },
	system: { label: 'System status', description: 'Host identity, uptime, and service health.' },
	summary: { label: 'Resource summary', description: 'CPU, memory, temperature, and total storage.' },
	operations: { label: 'Active operations', description: 'Scrubs, evacuations, and reconciliation work.' },
	storage: { label: 'Compact storage', description: 'Filesystems and member devices in a dense table.' },
	history: { label: 'CPU and memory history', description: 'Resource history for the selected time range.' },
	network: { label: 'Network', description: 'Interface status, throughput, and traffic history.' },
	disk_io: { label: 'Disk I/O', description: 'Per-device read and write activity.' },
};

const defaultCustomWidgets: DashboardWidgetConfig[] = [
	{ id: 'alerts', visible: true, width: 'full' },
	{ id: 'system', visible: true, width: 'full' },
	{ id: 'summary', visible: true, width: 'full' },
	{ id: 'operations', visible: true, width: 'full' },
	{ id: 'storage', visible: true, width: 'full' },
	{ id: 'history', visible: true, width: 'full' },
	{ id: 'network', visible: true, width: 'half' },
	{ id: 'disk_io', visible: true, width: 'half' },
];

type FixedPreset = Exclude<DashboardPreset, 'custom'>;

export const dashboardPresets: Record<FixedPreset, {
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

export function defaultDashboardPreferences(): DashboardPreferences {
	return {
		version: DASHBOARD_PREFERENCES_VERSION,
		preset: 'overview',
		density: 'comfortable',
		widgets: cloneWidgets(defaultCustomWidgets),
	};
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizeDashboardPreferences(value: unknown): DashboardPreferences {
	const fallback = defaultDashboardPreferences();
	if (!isRecord(value) || value.version !== DASHBOARD_PREFERENCES_VERSION) return fallback;

	const preset = value.preset;
	if (preset !== 'overview' && preset !== 'storage' && preset !== 'monitoring' && preset !== 'custom') {
		return fallback;
	}
	const density = value.density === 'compact' ? 'compact' : 'comfortable';
	const configured = Array.isArray(value.widgets) ? value.widgets : [];
	const byId = new Map<DashboardWidgetId, DashboardWidgetConfig>();
	for (const candidate of configured) {
		if (!isRecord(candidate) || !dashboardWidgetIds.includes(candidate.id as DashboardWidgetId)) continue;
		const id = candidate.id as DashboardWidgetId;
		if (byId.has(id)) continue;
		byId.set(id, {
			id,
			visible: candidate.visible !== false,
			width: candidate.width === 'half' ? 'half' : 'full',
		});
	}

	const widgets = [...byId.values()];
	for (const widget of defaultCustomWidgets) {
		if (!byId.has(widget.id)) widgets.push({ ...widget });
	}

	return { version: DASHBOARD_PREFERENCES_VERSION, preset, density, widgets };
}

export function parseDashboardPreferences(raw: string | null): DashboardPreferences {
	if (!raw) return defaultDashboardPreferences();
	try {
		return normalizeDashboardPreferences(JSON.parse(raw));
	} catch {
		return defaultDashboardPreferences();
	}
}

export function resolveDashboardWidgets(preferences: DashboardPreferences): DashboardWidgetConfig[] {
	const widgets = preferences.preset === 'custom'
		? preferences.widgets
		: dashboardPresets[preferences.preset].widgets;
	return cloneWidgets(widgets).filter((widget) => widget.visible);
}

export function resolveDashboardDensity(preferences: DashboardPreferences): DashboardDensity {
	return preferences.preset === 'custom'
		? preferences.density
		: dashboardPresets[preferences.preset].density;
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
	let value = $state<DashboardPreferences>(parseDashboardPreferences(
		typeof localStorage !== 'undefined' ? localStorage.getItem(DASHBOARD_PREFERENCES_KEY) : null
	));

	function set(next: DashboardPreferences) {
		value = normalizeDashboardPreferences(next);
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem(DASHBOARD_PREFERENCES_KEY, JSON.stringify(value));
		}
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
