import { describe, expect, test, vi } from 'vitest';
import type { BootStatus } from './types';
import {
	fetchBootStatus,
	isBootStatus,
	pollEngineUntilReady,
	waitingBootStatus,
} from './engine-startup';

const ready: BootStatus = {
	overall: 'ready',
	phases: [],
	process_started_at_unix: 1,
	ready_at_ms: 20,
};

describe('engine startup polling', () => {
	test('keeps polling after an initial gateway failure', async () => {
		const fetcher = vi.fn()
			.mockResolvedValueOnce(new Response('', { status: 502 }))
			.mockResolvedValueOnce(new Response(JSON.stringify(ready), { status: 200 }));
		const statuses: BootStatus[] = [];
		const result = await pollEngineUntilReady({
			fetcher,
			onStatus: (status) => statuses.push(status),
			signal: new AbortController().signal,
			wait: async () => true,
		});

		expect(result).toEqual(ready);
		expect(fetcher).toHaveBeenCalledTimes(2);
		expect(statuses.map((status) => status.overall)).toEqual(['booting', 'ready']);
	});

	test('keeps polling after a network failure', async () => {
		const fetcher = vi.fn()
			.mockRejectedValueOnce(new TypeError('network down'))
			.mockResolvedValueOnce(new Response(JSON.stringify(ready), { status: 200 }));
		const result = await pollEngineUntilReady({
			fetcher,
			onStatus: vi.fn(),
			signal: new AbortController().signal,
			wait: async () => true,
		});

		expect(result).toEqual(ready);
		expect(fetcher).toHaveBeenCalledTimes(2);
	});

	test('stops without fetching when cancelled', async () => {
		const controller = new AbortController();
		controller.abort();
		const fetcher = vi.fn();
		const result = await pollEngineUntilReady({
			fetcher,
			onStatus: vi.fn(),
			signal: controller.signal,
		});

		expect(result).toBeNull();
		expect(fetcher).not.toHaveBeenCalled();
	});

	test('stops while waiting to retry when cancelled', async () => {
		const controller = new AbortController();
		const fetcher = vi.fn().mockResolvedValue(new Response('', { status: 502 }));
		const polling = pollEngineUntilReady({
			fetcher,
			onStatus: vi.fn(),
			signal: controller.signal,
			retryMs: 60_000,
		});

		await vi.waitFor(() => expect(fetcher).toHaveBeenCalledOnce());
		controller.abort();

		await expect(polling).resolves.toBeNull();
		expect(fetcher).toHaveBeenCalledOnce();
	});

	test('rejects malformed successful responses', async () => {
		const fetcher = vi.fn().mockResolvedValue(
			new Response(JSON.stringify({ overall: 'ready' }), { status: 200 }),
		);
		expect(await fetchBootStatus(fetcher)).toBeNull();
		expect(isBootStatus({ overall: 'surprise', phases: [] })).toBe(false);
	});

	test('creates a fail-closed waiting snapshot', () => {
		expect(waitingBootStatus(12_345)).toEqual({
			overall: 'booting',
			phases: [],
			process_started_at_unix: 12,
			ready_at_ms: null,
		});
	});
});
