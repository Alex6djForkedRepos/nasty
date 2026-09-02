import { render } from 'svelte/server';
import { describe, expect, test } from 'vitest';
import AlertsWidget from './alerts-widget.svelte';
import HealthWidget from './health-widget.svelte';
import HistoryWidget from './history-widget.svelte';
import OperationsWidget from './operations-widget.svelte';
import SummaryWidget from './summary-widget.svelte';
import SystemWidget from './system-widget.svelte';
import type { ActiveAlert, Filesystem, SystemInfo, SystemStats } from '$lib/types';

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
		expect(body).toContain('xl:grid-cols-1');
		expect(body).not.toContain('role="img"');
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
