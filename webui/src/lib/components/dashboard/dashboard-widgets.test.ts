import { render } from 'svelte/server';
import { describe, expect, test } from 'vitest';
import AlertsWidget from './alerts-widget.svelte';
import ClockWidget from './clock-widget.svelte';
import ComputeWidget from './compute-widget.svelte';
import HealthWidget from './health-widget.svelte';
import HistoryWidget from './history-widget.svelte';
import OperationsWidget from './operations-widget.svelte';
import ScheduleWidget from './schedule-widget.svelte';
import SummaryWidget from './summary-widget.svelte';
import SystemWidget from './system-widget.svelte';
import WidgetOptions from './widget-options.svelte';
import type { ActiveAlert, AppsStatus, BackupScheduleEntry, Filesystem, Settings, SystemInfo, SystemStats, VmStatus } from '$lib/types';

const alert = (severity: ActiveAlert['severity'], message: string): ActiveAlert => ({
	rule_id: message,
	rule_name: message,
	severity,
	metric: 'cpu_load_percent',
	message,
	current_value: 90,
	threshold: 80,
	source: 'system',
	instance_id: message,
	acknowledged: false,
	acknowledged_at: null,
	acknowledged_by: null,
});

const info: SystemInfo = {
	hostname: 'nasty',
	version: '1.2.3',
	uptime_seconds: 3600,
	kernel: '6.18.0',
	bcachefs_version: '1.39.4',
	bcachefs_commit: null,
	bcachefs_pinned_ref: null,
	bcachefs_recommended_ref: null,
	bcachefs_is_custom: false,
	timezone: 'UTC',
	ntp_synced: true,
	current_time: '2026-09-02T20:00:00Z',
};

const stats: SystemStats = {
	cpu: { count: 4, load_1: 1, load_5: 0.75, load_15: 0.5, temp_c: 42, freq_mhz: 2400, governor: 'powersave' },
	memory: { total_bytes: 1000, used_bytes: 500, available_bytes: 500, swap_total_bytes: 0, swap_used_bytes: 0, bcachefs_btree_cache_bytes: 100 },
	network: [],
	disk_io: [],
};

const filesystem: Filesystem = {
	name: 'pool',
	uuid: 'pool-uuid',
	devices: [],
	mount_point: '/mnt/pool',
	mounted: true,
	total_bytes: 400,
	used_bytes: 100,
	available_bytes: 300,
	options: {} as Filesystem['options'],
};

const appsStatus: AppsStatus = {
	enabled: true,
	running: true,
	app_count: 2,
	storage_ok: true,
};

const settings = {
	timezone: 'UTC',
	clock_24h: true,
	dashboard_motd: 'Storage maintenance tonight.',
} as Settings;

const vm = (name: string, running: boolean, cpus: number, memory_mib: number): VmStatus => ({
	id: name,
	name,
	cpus,
	memory_mib,
	disks: [],
	networks: [],
	passthrough_devices: [],
	cdroms: [],
	boot_order: 'disk',
	uefi: true,
	autostart: false,
	running,
});

describe('tiny dashboard widgets', () => {
	test('keeps critical alert and active operation states explicit', () => {
		const alerts = render(AlertsWidget, {
			props: {
				alerts: [alert('warning', 'Warm disk'), alert('critical', 'Hot disk')],
				loaded: true,
				density: 'compact',
				presentation: 'tiny',
			},
		}).body;
		const operations = render(OperationsWidget, {
			props: {
				operations: [{ kind: 'scrub', fs: 'pool', detail: 'Checking pool', progress_percent: 42 }],
				loaded: true,
				density: 'compact',
				presentation: 'tiny',
			},
		}).body;

		expect(alerts).toContain('2 active alerts - 1 critical');
		expect(alerts).toContain('Open alerts');
		expect(operations).toContain('1 active - Scrub on pool');
		expect(operations).toContain('Open operations');
	});

	test('reports unavailable and down system states without relying on color', () => {
		const unavailable = render(SystemWidget, {
			props: { info: null, health: null, infoLoaded: false, healthLoaded: false, density: 'compact', presentation: 'tiny' },
		}).body;
		const healthUnavailable = render(SystemWidget, {
			props: { info, health: null, infoLoaded: true, healthLoaded: false, density: 'compact', presentation: 'tiny' },
		}).body;
		const infoUnavailable = render(SystemWidget, {
			props: { info: null, health: { status: 'ok', services: [] }, infoLoaded: false, healthLoaded: true, density: 'compact', presentation: 'tiny' },
		}).body;
		const unhealthy = render(SystemWidget, {
			props: {
				info,
				health: { status: 'critical', services: [{ name: 'Engine', running: false }] },
				infoLoaded: true,
				healthLoaded: true,
				density: 'compact',
				presentation: 'tiny',
			},
		}).body;

		expect(unavailable).toContain('System status unavailable');
		expect(healthUnavailable).toContain('Health unavailable');
		expect(infoUnavailable).toContain('System info unavailable');
		expect(unhealthy).toContain('1 service down');
	});

	test('renders current and peak history values instead of full charts', () => {
		const body = render(HistoryWidget, {
			props: {
				cpuSamples: [
					{ time: new Date('2026-09-01T10:00:00Z'), in: 12, out: 0 },
					{ time: new Date('2026-09-01T10:01:00Z'), in: 8, out: 0 },
				],
				memorySamples: [
					{ time: new Date('2026-09-01T10:00:00Z'), in: 40, out: 0 },
					{ time: new Date('2026-09-01T10:01:00Z'), in: 45, out: 0 },
				],
				range: '5m',
				loading: false,
				width: 'quarter',
				density: 'compact',
				presentation: 'tiny',
			},
		}).body;

		expect(body).toContain('8.0%');
		expect(body).toContain('5m peak 12.0%');
		expect(body).toContain('45.0%');
		expect(body).toContain('background-color: var(--color-emerald-500)');
		expect(body).toContain('xl:grid-cols-1');
		expect(body).not.toContain('role="img"');
	});

	test('colors memory usage by current severity', () => {
		const renderMemory = (current: number) => render(HistoryWidget, {
			props: {
				cpuSamples: [],
				memorySamples: [
					{ time: new Date('2026-09-01T10:00:00Z'), in: 95, out: 0 },
					{ time: new Date('2026-09-01T10:01:00Z'), in: current, out: 0 },
				],
				range: '5m',
				loading: false,
				width: 'quarter' as const,
				density: 'compact' as const,
				presentation: 'tiny' as const,
			},
		}).body;

		expect(renderMemory(45)).toContain('background-color: var(--color-emerald-500)');
		expect(renderMemory(80)).toContain('background-color: var(--color-amber-500)');
		expect(renderMemory(95)).toContain('background-color: var(--color-red-500)');
	});
});

describe('dashboard health widget', () => {
	test('renders service and managed container health as independent linked cards', () => {
		const services = render(HealthWidget, {
			props: {
				kind: 'services',
				services: { enabled: 3, running: 2 },
				freshness: 'current',
				density: 'comfortable',
			},
		}).body;
		const containers = render(HealthWidget, {
			props: {
				kind: 'containers',
				containers: { runtime: 'running', expected: 2, running: 2 },
				freshness: 'current',
				density: 'comfortable',
			},
		}).body;

		expect(services).toContain('href="/services"');
		expect(services).toContain('3 enabled');
		expect(services).toContain('2 running');
		expect(services).toContain('1 enabled service not running.');
		expect(services).not.toContain('href="/apps"');
		expect(containers).toContain('href="/apps"');
		expect(containers).toContain('2 expected');
		expect(containers).toContain('All expected containers are running.');
		expect(containers).toContain('Simple apps count once; Compose service instances count individually.');
		expect(containers).not.toContain('href="/services"');
	});

	test('distinguishes disabled, loading, unavailable, and stale states in text', () => {
		const loading = render(HealthWidget, {
			props: {
				kind: 'services',
				services: null,
				freshness: 'loading',
				density: 'compact',
			},
		}).body;
		const disabled = render(HealthWidget, {
			props: {
				kind: 'containers',
				containers: { runtime: 'disabled', expected: null, running: null },
				freshness: 'current',
				density: 'compact',
			},
		}).body;
		const stale = render(HealthWidget, {
			props: {
				kind: 'services',
				services: { enabled: 1, running: 1 },
				freshness: 'stale',
				density: 'compact',
			},
		}).body;
		const refreshing = render(HealthWidget, {
			props: {
				kind: 'containers',
				containers: { runtime: 'running', expected: 1, running: 1 },
				freshness: 'refreshing',
				density: 'compact',
			},
		}).body;

		expect(loading).toContain('Checking enabled services.');
		expect(disabled).toContain('Docker runtime is disabled.');
		expect(disabled).toContain('px-4 py-3');
		expect(stale).toContain('Stale - healthy');
		expect(stale).toContain('Refresh failed; showing last known healthy state.');
		expect(stale).toContain('role="status"');
		expect(refreshing).toContain('Refreshing - healthy');
		expect(refreshing).toContain('Refreshing; showing last known healthy state.');
		expect(refreshing).not.toContain('Refresh failed');
	});
});

describe('dashboard compute widget', () => {
	test('summarizes active VM allocations and Docker workloads with destination links', () => {
		const body = render(ComputeWidget, {
			props: {
				vms: [vm('router', true, 2, 2048), vm('lab', false, 4, 4096)],
				vmFreshness: 'current',
				appsStatus,
				containers: { runtime: 'running', expected: 4, running: 3 },
				containerFreshness: 'current',
				density: 'comfortable',
			},
		}).body;

		expect(body).toContain('href="/vms"');
		expect(body).toContain('1 <span class="text-base font-medium text-muted-foreground">/ 2</span>');
		expect(body).toContain('2 vCPU - 2.0 GiB active');
		expect(body).toContain('href="/apps"');
		expect(body).toContain('3 / 4');
		expect(body).toContain('2 apps - containers running');
	});

	test('keeps runtime and freshness failures explicit', () => {
		const disabled = render(ComputeWidget, {
			props: {
				vms: [],
				vmFreshness: 'stale',
				appsStatus: { ...appsStatus, enabled: false, running: false, app_count: 0 },
				containers: { runtime: 'disabled', expected: null, running: null },
				containerFreshness: 'current',
				density: 'compact',
			},
		}).body;
		const unavailable = render(ComputeWidget, {
			props: {
				vms: null,
				vmFreshness: 'unavailable',
				appsStatus: null,
				containers: null,
				containerFreshness: 'unavailable',
				density: 'compact',
			},
		}).body;

		expect(disabled).toContain('Stale');
		expect(disabled).toContain('Data stale.');
		expect(disabled).toContain('No VMs defined');
		expect(disabled).toContain('Docker runtime is disabled.');
		expect(unavailable).toContain('VM inventory could not be loaded.');
		expect(unavailable).toContain('Docker status could not be loaded.');
		expect(unavailable).toContain('role="status"');
	});

	test('distinguishes loading and partial Docker inventory from failures', () => {
		const loading = render(ComputeWidget, {
			props: {
				vmFreshness: 'loading',
				containerFreshness: 'loading',
				density: 'compact',
			},
		}).body;
		const partial = render(ComputeWidget, {
			props: {
				vms: [],
				vmFreshness: 'current',
				appsStatus,
				containers: { runtime: 'running', expected: null, running: null },
				containerFreshness: 'unavailable',
				density: 'compact',
			},
		}).body;

		expect(loading).toContain('Checking VM inventory.');
		expect(loading).toContain('Checking Docker runtime.');
		expect(loading).not.toContain('could not be loaded');
		expect(partial).toContain('Inventory unavailable');
		expect(partial).toContain('2 configured apps.');
	});
});

describe('dashboard clock and schedule widgets', () => {
	test('renders host time state and the configured dashboard notice', () => {
		const body = render(ClockWidget, {
			props: { info, settings, loaded: true, density: 'comfortable' },
		}).body;

		expect(body).toContain('Analog clock showing');
		expect(body).toContain('UTC');
		expect(body).toContain('NTP synchronized');
		expect(body).toContain('Storage maintenance tonight.');
	});

	test('uses the configured timezone when runtime timezone detection falls back to UTC', () => {
		const body = render(ClockWidget, {
			props: {
				info: { ...info, current_time: '2026-09-02T02:00:00.500Z', timezone: 'UTC' },
				settings: { ...settings, timezone: 'America/New_York' },
				loaded: true,
				density: 'comfortable',
			},
		}).body;

		expect(body).toContain('America/New_York');
		expect(body).toContain('rotate(300 50 50)');
	});

	test('does not substitute browser time when host time is unavailable', () => {
		const legacyInfo = { ...info, current_time: undefined } as unknown as SystemInfo;
		const body = render(ClockWidget, {
			props: { info: legacyInfo, settings, loaded: true, density: 'comfortable' },
		}).body;

		expect(body).toContain('Host time is unavailable.');
		expect(body).not.toContain('Analog clock showing');
	});

	test('renders upcoming and invalid backup schedules explicitly', () => {
		const entries: BackupScheduleEntry[] = [
			{
				profile_id: 'daily',
				profile_name: 'Daily data',
				schedule: '0 3 * * *',
				next_run_at: '2099-09-03T03:00:00+00:00',
				schedule_error: null,
				last_run: null,
			},
			{
				profile_id: 'broken',
				profile_name: 'Broken profile',
				schedule: 'not cron',
				next_run_at: null,
				schedule_error: 'invalid cron',
				last_run: null,
			},
		];
		const body = render(ScheduleWidget, {
			props: { entries, currentTime: info.current_time, freshness: 'current', density: 'comfortable' },
		}).body;

		expect(body).toContain('Upcoming backups');
		expect(body).toContain('Daily data');
		expect(body).toContain('Broken profile');
		expect(body).toContain('Invalid schedule');
		expect(body).toContain('href="/backups"');
	});
});

describe('dashboard resource summary widgets', () => {
	test('renders each resource as an independent card', () => {
		const cpu = render(SummaryWidget, { props: { kind: 'cpu_load', stats, density: 'comfortable' } }).body;
		const memory = render(SummaryWidget, { props: { kind: 'memory_usage', stats, density: 'comfortable' } }).body;
		const status = render(SummaryWidget, { props: { kind: 'cpu_status', stats, density: 'comfortable' } }).body;
		const storage = render(SummaryWidget, { props: { kind: 'storage_summary', filesystems: [filesystem], density: 'comfortable' } }).body;

		expect(cpu).toContain('CPU load');
		expect(cpu).toContain('1.00');
		expect(cpu).not.toContain('Memory');
		expect(memory).toContain('Memory');
		expect(memory).toContain('Btree cache');
		expect(memory).not.toContain('CPU load');
		expect(status).toContain('2.4 GHz - powersave');
		expect(storage).toContain('Storage');
		expect(storage).toContain('1 filesystem');
	});

	test('keeps unavailable standalone resources visible', () => {
		const cpu = render(SummaryWidget, { props: { kind: 'cpu_load', stats: null, density: 'compact' } }).body;
		const status = render(SummaryWidget, {
			props: { kind: 'cpu_status', stats: { ...stats, cpu: { ...stats.cpu, temp_c: null, freq_mhz: null } }, density: 'compact' },
		}).body;
		const storage = render(SummaryWidget, { props: { kind: 'storage_summary', filesystems: [], density: 'compact' } }).body;
		const unavailableStorage = render(SummaryWidget, { props: { kind: 'storage_summary', filesystemsLoaded: false, density: 'compact' } }).body;

		expect(cpu).toContain('Unavailable');
		expect(status).toContain('CPU temperature and frequency are unavailable.');
		expect(storage).toContain('No filesystems');
		expect(unavailableStorage).toContain('Filesystem inventory could not be loaded.');
	});
});

describe('dashboard widget options', () => {
	test('offers supported widths and presentation modes', () => {
		const configurable = render(WidgetOptions, {
			props: {
				widget: { id: 'alerts', visible: true, width: 'quarter', presentation: 'tiny', column: 0, row: 0, priority: 0 },
				onChange: () => undefined,
			},
		}).body;
		const standardOnly = render(WidgetOptions, {
			props: {
				widget: { id: 'storage', visible: true, width: 'full', presentation: 'standard', column: 0, row: 0, priority: 0 },
				onChange: () => undefined,
			},
		}).body;

		expect(configurable).toContain('Configure Alerts widget');
		expect(configurable).toContain('1/4 width');
		expect(configurable).toContain('Presentation');
		expect(configurable).toContain('Tiny');
		expect(standardOnly).toContain('Full width');
		expect(standardOnly).toContain('1/2 width');
		expect(standardOnly).not.toContain('1/4 width');
		expect(standardOnly).not.toContain('Presentation');
	});
});
