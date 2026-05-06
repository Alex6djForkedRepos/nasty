import { describe, expect, test } from 'vitest';
import { createIoHistory } from './history.svelte';

const t = (s: number) => new Date(2026, 0, 1, 0, 0, s);

describe('createIoHistory', () => {
	test('push adds samples to the named resource', () => {
		const h = createIoHistory();
		h.push('sda', t(1), 100, 50);
		h.push('sda', t(2), 200, 100);
		const samples = h.getSamples('sda');
		expect(samples).toHaveLength(2);
		expect(samples[0]).toEqual({ time: t(1), in: 100, out: 50 });
		expect(samples[1]).toEqual({ time: t(2), in: 200, out: 100 });
	});

	test('separate resources keep separate buffers', () => {
		const h = createIoHistory();
		h.push('sda', t(1), 1, 1);
		h.push('sdb', t(1), 2, 2);
		h.push('sda', t(2), 3, 3);
		expect(h.getSamples('sda')).toHaveLength(2);
		expect(h.getSamples('sdb')).toHaveLength(1);
	});

	test('getSamples for an unknown resource returns an empty array', () => {
		const h = createIoHistory();
		expect(h.getSamples('missing')).toEqual([]);
	});

	test('buffer truncates oldest samples once it exceeds 400 entries', () => {
		const h = createIoHistory();
		for (let i = 0; i < 500; i++) {
			h.push('sda', t(i), i, i);
		}
		const samples = h.getSamples('sda');
		// Cap is 400; we pushed 500 → oldest 100 dropped, newest 400 kept.
		expect(samples).toHaveLength(400);
		expect(samples[0].in).toBe(100); // sample #100 is now the oldest
		expect(samples[399].in).toBe(499); // newest is the last one we pushed
	});

	test('clear empties every resource', () => {
		const h = createIoHistory();
		h.push('sda', t(1), 1, 1);
		h.push('sdb', t(1), 2, 2);
		h.clear();
		expect(h.getSamples('sda')).toEqual([]);
		expect(h.getSamples('sdb')).toEqual([]);
		expect(h.resources.size).toBe(0);
	});
});
