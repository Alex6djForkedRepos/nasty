import { afterEach, describe, expect, test } from 'vitest';
import {
	confirmDangerous,
	confirmDangerousRespond,
	confirmDangerousState
} from './confirm-dangerous.svelte';

afterEach(() => {
	confirmDangerousState.open = false;
	confirmDangerousState.resolve = null;
});

describe('confirmDangerous', () => {
	test('opens the dialog and populates title, message, and expectedValue', () => {
		void confirmDangerous('Delete tank?', 'Type "tank" to confirm', 'tank');
		expect(confirmDangerousState.open).toBe(true);
		expect(confirmDangerousState.title).toBe('Delete tank?');
		expect(confirmDangerousState.message).toBe('Type "tank" to confirm');
		expect(confirmDangerousState.expectedValue).toBe('tank');
	});

	test('confirmDangerousRespond(true) resolves with true and closes', async () => {
		const p = confirmDangerous('x', 'y', 'z');
		confirmDangerousRespond(true);
		await expect(p).resolves.toBe(true);
		expect(confirmDangerousState.open).toBe(false);
	});

	test('confirmDangerousRespond(false) resolves with false', async () => {
		const p = confirmDangerous('x', 'y', 'z');
		confirmDangerousRespond(false);
		await expect(p).resolves.toBe(false);
	});

	test('a second response after the promise resolves is a no-op', () => {
		void confirmDangerous('x', 'y', 'z');
		confirmDangerousRespond(true);
		expect(() => confirmDangerousRespond(true)).not.toThrow();
	});
});
