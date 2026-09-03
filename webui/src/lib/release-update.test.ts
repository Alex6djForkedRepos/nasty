import { afterEach, describe, expect, test, vi } from 'vitest';
import type { UpdateInfo } from './types';
import { getReleaseUpdateSnapshot, invalidateReleaseUpdateCheck, publishReleaseUpdate, releaseUpdateDisplay, requestReleaseUpdateCheck, setReleaseUpdateSnapshot, shouldCheckReleaseUpdate } from './release-update';

const info: UpdateInfo = {
	current_version: '1.2.3',
	latest_version: '1.2.4',
	update_available: null,
	channel: 'mild',
	last_attempt: null,
	error: null,
	inputs: null,
};

describe('release update display', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

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

	test('retains the latest check state for routes mounted after it completes', () => {
		setReleaseUpdateSnapshot(null, 'loading');
		expect(getReleaseUpdateSnapshot()).toEqual({ info: null, requestState: 'loading' });

		const available = { ...info, update_available: true };
		publishReleaseUpdate(available, 'ready');
		expect(getReleaseUpdateSnapshot()).toEqual({ info: available, requestState: 'ready' });

		publishReleaseUpdate(null, 'idle');
		expect(getReleaseUpdateSnapshot()).toEqual({ info: null, requestState: 'idle' });
	});

	test('deduplicates concurrent remote checks', async () => {
		let resolve!: (value: typeof info) => void;
		let calls = 0;
		let timeout = 0;
		let method = '';
		const client = {
			call: <T>(nextMethod: string, _params?: unknown, timeoutMs?: number) => {
				calls++;
				method = nextMethod;
				timeout = timeoutMs ?? 0;
				return new Promise<typeof info>((done) => { resolve = done; }) as Promise<T>;
			},
		};
		const first = requestReleaseUpdateCheck(client);
		const second = requestReleaseUpdateCheck(client);

		expect(second).toBe(first);
		expect(calls).toBe(1);
		expect(method).toBe('system.update.check');
		expect(timeout).toBe(180_000);
		resolve(info);
		await first;
		await Promise.resolve();
		const third = requestReleaseUpdateCheck(client);
		expect(calls).toBe(2);
		resolve(info);
		await third;
	});

	test('uses the REST gateway for cached background polling', async () => {
		const fetchMock = vi.fn().mockResolvedValue({
			ok: true,
			json: () => Promise.resolve(info),
		});
		vi.stubGlobal('fetch', fetchMock);
		const client = { call: <T>() => Promise.resolve(info) as Promise<T> };

		await requestReleaseUpdateCheck(client, 'cached');
		expect(fetchMock).toHaveBeenCalledWith(
			'/api/v1/system/update/check_cached',
			expect.objectContaining({ signal: expect.any(AbortSignal) }),
		);
	});

	test('starts a fresh check after the previous request is invalidated', async () => {
		const resolvers: ((value: UpdateInfo) => void)[] = [];
		let calls = 0;
		const client = {
			call: <T>() => {
				calls++;
				return new Promise<UpdateInfo>((resolve) => { resolvers.push(resolve); }) as Promise<T>;
			},
		};
		const stale = requestReleaseUpdateCheck(client);
		invalidateReleaseUpdateCheck();
		const fresh = requestReleaseUpdateCheck(client);

		expect(fresh).not.toBe(stale);
		expect(calls).toBe(2);
		resolvers[0](info);
		await stale;
		expect(requestReleaseUpdateCheck(client)).toBe(fresh);
		resolvers[1](info);
		await fresh;
	});
});
