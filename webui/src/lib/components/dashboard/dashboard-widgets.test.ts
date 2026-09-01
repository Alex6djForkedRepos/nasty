import { render } from 'svelte/server';
import { describe, expect, test } from 'vitest';
import AlertsWidget from './alerts-widget.svelte';
import HealthWidget from './health-widget.svelte';
import HistoryWidget from './history-widget.svelte';
import OperationsWidget from './operations-widget.svelte';
import SystemWidget from './system-widget.svelte';
import type { ActiveAlert, SystemInfo } from '$lib/types';

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
				width: 'half',
				density: 'compact',
				presentation: 'tiny',
			},
		}).body;

		expect(body).toContain('8.0%');
		expect(body).toContain('5m peak 12.0%');
		expect(body).toContain('45.0%');
		expect(body).not.toContain('role="img"');
	});
});

describe('dashboard health widget', () => {
	test('renders linked service and managed container counts with explicit health states', () => {
		const body = render(HealthWidget, {
			props: {
				services: { enabled: 3, running: 2 },
				servicesFreshness: 'current',
				containers: { runtime: 'running', expected: 2, running: 2 },
				containersFreshness: 'current',
				density: 'comfortable',
				width: 'full',
			},
		}).body;

		expect(body).toContain('href="/services"');
		expect(body).toContain('3 enabled');
		expect(body).toContain('2 running');
		expect(body).toContain('1 enabled service not running.');
		expect(body).toContain('href="/apps"');
		expect(body).toContain('2 expected');
		expect(body).toContain('All expected containers are running.');
		expect(body).toContain('Simple apps count once; Compose service instances count individually.');
		expect(body).toContain('md:grid-cols-2');
		expect(body).toContain('px-5 py-4');
	});

	test('distinguishes disabled, loading, unavailable, and stale states in text', () => {
		const disabled = render(HealthWidget, {
			props: {
				services: null,
				servicesFreshness: 'loading',
				containers: { runtime: 'disabled', expected: null, running: null },
				containersFreshness: 'current',
				density: 'compact',
				width: 'half',
			},
		}).body;
		const stale = render(HealthWidget, {
			props: {
				services: { enabled: 1, running: 1 },
				servicesFreshness: 'stale',
				containers: null,
				containersFreshness: 'unavailable',
				density: 'compact',
				width: 'half',
			},
		}).body;
		const refreshing = render(HealthWidget, {
			props: {
				services: { enabled: 1, running: 1 },
				servicesFreshness: 'refreshing',
				containers: { runtime: 'running', expected: 1, running: 1 },
				containersFreshness: 'refreshing',
				density: 'compact',
				width: 'half',
			},
		}).body;

		expect(disabled).toContain('Checking enabled services.');
		expect(disabled).toContain('Docker runtime is disabled.');
		expect(disabled).toContain('px-4 py-3');
		expect(disabled).not.toContain('md:grid-cols-2');
		expect(stale).toContain('Stale - healthy');
		expect(stale).toContain('Refresh failed; showing last known healthy state.');
		expect(stale).toContain('Managed container health could not be loaded.');
		expect(stale).toContain('role="status"');
		expect(refreshing).toContain('Refreshing - healthy');
		expect(refreshing).toContain('Refreshing; showing last known healthy state.');
		expect(refreshing).not.toContain('Refresh failed');
	});
});
