import { describe, expect, test } from 'vitest';
import type { App, AppsStatus, ProtocolStatus } from '$lib/types';
import {
	shouldPollDashboardHealth,
	summarizeEnabledServices,
	summarizeManagedContainers,
} from './dashboard-health';

const appsStatus = (overrides: Partial<AppsStatus> = {}): AppsStatus => ({
	enabled: true,
	running: true,
	app_count: 0,
	storage_ok: true,
	...overrides,
});

const app = (name: string, overrides: Partial<App> = {}): App => ({
	name,
	image: `${name}:latest`,
	status: 'running',
	created: '2026-09-01T00:00:00Z',
	kind: 'simple',
	...overrides,
});

describe('dashboard health summaries', () => {
	test('counts only enabled services as expected or running', () => {
		const protocols: ProtocolStatus[] = [
			{ name: 'ssh', display_name: 'SSH', enabled: true, running: true, system_service: true },
			{ name: 'smb', display_name: 'SMB', enabled: true, running: false, system_service: false },
			{ name: 'nvmeof', display_name: 'NVMe-oF', enabled: false, running: true, system_service: false },
		];

		expect(summarizeEnabledServices(protocols)).toEqual({ enabled: 2, running: 1 });
	});

	test('counts configured simple and compose containers without counting stack rows', () => {
		const apps: App[] = [
			app('simple-running'),
			app('simple-stopped', { status: 'exited' }),
			app('compose-running', {
				kind: 'compose',
				expected_containers: ['web', 'db'],
				containers: [
					{ name: 'web', container_id: '1', image: 'web', status: 'running' },
					{ name: 'db', container_id: '2', image: 'db', status: 'running' },
				],
			}),
			app('compose-partial', {
				kind: 'compose',
				expected_containers: ['web', 'db'],
				containers: [
					{ name: 'web', container_id: '3', image: 'web', status: 'running' },
					{ name: 'db', container_id: '4', image: 'db', status: 'exited' },
				],
			}),
			app('compose-empty', { kind: 'compose', expected_containers: ['worker'], containers: [] }),
		];

		expect(summarizeManagedContainers(appsStatus(), apps)).toEqual({
			runtime: 'running',
			expected: 7,
			running: 4,
		});
	});

	test('treats a disabled runtime as neutral and an enabled stopped runtime as down', () => {
		expect(summarizeManagedContainers(appsStatus({ enabled: false, running: false }), [app('offline')])).toEqual({
			runtime: 'disabled',
			expected: null,
			running: null,
		});
		expect(summarizeManagedContainers(appsStatus({ running: false }), [app('offline')])).toEqual({
			runtime: 'down',
			expected: 1,
			running: 0,
		});
	});

	test('marks compose expectations unknown when an older inventory omits service names', () => {
		expect(summarizeManagedContainers(appsStatus(), [app('compose', { kind: 'compose', containers: [] })])).toEqual({
			runtime: 'running',
			expected: null,
			running: null,
		});
	});

	test('treats an explicit zero-replica compose expectation as known', () => {
		expect(summarizeManagedContainers(appsStatus(), [app('compose', {
			kind: 'compose',
			expected_containers: [],
			containers: [],
		})])).toEqual({
			runtime: 'running',
			expected: 0,
			running: 0,
		});
	});

	test('counts a configured compose service as stopped when its container is absent', () => {
		const compose = app('compose', {
			kind: 'compose',
			expected_containers: ['web', 'db'],
			containers: [{ name: 'web', container_id: '1', image: 'web', status: 'running' }],
		});

		expect(summarizeManagedContainers(appsStatus(), [compose])).toEqual({
			runtime: 'running',
			expected: 2,
			running: 1,
		});
	});

	test('matches running compose replicas to expected instances', () => {
		const compose = app('scaled', {
			kind: 'compose',
			expected_containers: ['web', 'web', 'web'],
			containers: [
				{ name: 'web', container_id: '1', image: 'web', status: 'running' },
				{ name: 'web', container_id: '2', image: 'web', status: 'running' },
				{ name: 'web', container_id: '3', image: 'web', status: 'exited' },
			],
		});

		expect(summarizeManagedContainers(appsStatus(), [compose])).toEqual({
			runtime: 'running',
			expected: 3,
			running: 2,
		});
	});

	test('polls only when the widget and browser document are visible', () => {
		expect(shouldPollDashboardHealth(true, false)).toBe(true);
		expect(shouldPollDashboardHealth(false, false)).toBe(false);
		expect(shouldPollDashboardHealth(true, true)).toBe(false);
	});
});
