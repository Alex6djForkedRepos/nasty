import { describe, expect, test } from 'vitest';
import { releaseUpdateDisplay, requestReleaseUpdateCheck, shouldCheckReleaseUpdate } from './release-update';

const info = {
	current_version: '1.2.3',
	latest_version: '1.2.4',
	update_available: null,
	error: null,
};

describe('release update display', () => {
	test('distinguishes loading and unchecked states', () => {
		expect(releaseUpdateDisplay(null, 'loading')).toMatchObject({ kind: 'loading', label: 'checking' });
		expect(releaseUpdateDisplay(info, 'ready')).toMatchObject({ kind: 'unknown', label: 'unchecked' });
	});

	test('marks a confirmed current version', () => {
		expect(releaseUpdateDisplay({ ...info, update_available: false }, 'ready')).toEqual({
			kind: 'current',
			label: 'current',
			title: 'NASty 1.2.3 is current.',
		});
	});

	test('describes an available version and the Update page action', () => {
		expect(releaseUpdateDisplay({ ...info, update_available: true }, 'ready')).toEqual({
			kind: 'available',
			label: 'update',
			title: 'NASty update 1.2.4 is available. Open the Update page.',
		});
	});

	test('keeps transport and lookup failures distinct from current', () => {
		expect(releaseUpdateDisplay(null, 'failed')).toMatchObject({ kind: 'failed', label: 'check failed' });
		expect(releaseUpdateDisplay({ ...info, error: 'GitHub rate limited' }, 'ready')).toEqual({
			kind: 'failed',
			label: 'check failed',
			title: 'NASty update check failed: GitHub rate limited',
		});
	});

	test('requests a remote check only when local status is unknown', () => {
		expect(shouldCheckReleaseUpdate(info)).toBe(true);
		expect(shouldCheckReleaseUpdate({ ...info, update_available: false })).toBe(false);
		expect(shouldCheckReleaseUpdate({ ...info, update_available: true })).toBe(false);
		expect(shouldCheckReleaseUpdate({ ...info, error: 'offline' })).toBe(false);
	});

	test('deduplicates concurrent remote checks', async () => {
		let resolve!: (value: typeof info) => void;
		let calls = 0;
		let timeout = 0;
		const client = {
			call: <T>(_method: string, _params?: unknown, timeoutMs?: number) => {
				calls++;
				timeout = timeoutMs ?? 0;
				return new Promise<typeof info>((done) => { resolve = done; }) as Promise<T>;
			},
		};
		const first = requestReleaseUpdateCheck(client);
		const second = requestReleaseUpdateCheck(client);

		expect(second).toBe(first);
		expect(calls).toBe(1);
		expect(timeout).toBe(180_000);
		resolve(info);
		await first;
		await Promise.resolve();
		const third = requestReleaseUpdateCheck(client);
		expect(calls).toBe(2);
		resolve(info);
		await third;
	});
});
