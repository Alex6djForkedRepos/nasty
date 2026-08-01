import { describe, expect, test } from 'vitest';
import { appMonogram, normalizeImageRepository, resolveAppIcon } from './app-icons';

describe('app icons', () => {
	test('normalizes registries, tags, digests, and the Docker Hub library namespace', () => {
		expect(normalizeImageRepository('lscr.io/linuxserver/jellyfin:latest')).toBe('linuxserver/jellyfin');
		expect(normalizeImageRepository('docker.io/library/postgres@sha256:abc')).toBe('postgres');
		expect(normalizeImageRepository('registry.example:5000/team/My_App:v2')).toBe('team/my-app');
		expect(normalizeImageRepository('https://example.test/not-an-image')).toBe('');
	});

	test('matches known images without depending on the user-selected app name', () => {
		const icon = resolveAppIcon({
			name: 'media',
			image: 'lscr.io/linuxserver/jellyfin:latest',
			kind: 'simple',
			containers: []
		});
		expect(icon?.title).toBe('Jellyfin');
	});

	test('uses a compose project name before an arbitrary primary service image', () => {
		const icon = resolveAppIcon({
			name: 'immich',
			image: 'docker.io/library/redis:latest',
			kind: 'compose',
			containers: [{ name: 'cache', container_id: 'abc', image: 'redis:latest', status: 'running' }]
		});
		expect(icon?.title).toBe('Immich');
	});

	test('checks compose service images when the project and primary image are unknown', () => {
		const icon = resolveAppIcon({
			name: 'photos-stack',
			image: 'example/worker:latest',
			kind: 'compose',
			containers: [
				{ name: 'worker', container_id: 'abc', image: 'example/worker:latest', status: 'running' },
				{ name: 'web', container_id: 'def', image: 'ghcr.io/immich-app/immich-server:v2', status: 'running' }
			]
		});
		expect(icon?.title).toBe('Immich');
	});

	test('does not identify an ambiguous compose stack as one of its dependencies', () => {
		const icon = resolveAppIcon({
			name: 'photos-stack',
			image: 'redis:latest',
			kind: 'compose',
			containers: [
				{ name: 'cache', container_id: 'abc', image: 'redis:latest', status: 'running' },
				{ name: 'web', container_id: 'def', image: 'ghcr.io/immich-app/immich-server:v2', status: 'running' }
			]
		});
		expect(icon).toBeNull();
	});

	test('returns no brand for unknown apps and gives them a stable monogram', () => {
		expect(resolveAppIcon({ name: 'my-service', image: 'example/custom:1', kind: 'simple', containers: [] })).toBeNull();
		expect(appMonogram('my-service')).toEqual(appMonogram('my-service'));
		expect(appMonogram('my-service').initials).toBe('MS');
		expect(appMonogram('my-service').hue).toBeGreaterThanOrEqual(0);
		expect(appMonogram('my-service').hue).toBeLessThan(360);
	});
});
