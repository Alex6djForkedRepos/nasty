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
	test('opens the dialog and populates title, message, expectedValue, and the default label', () => {
		void confirmDangerous('Delete tank?', 'Type "tank" to confirm', 'tank');
		expect(confirmDangerousState.open).toBe(true);
		expect(confirmDangerousState.title).toBe('Delete tank?');
		expect(confirmDangerousState.message).toBe('Type "tank" to confirm');
		expect(confirmDangerousState.expectedValue).toBe('tank');
		expect(confirmDangerousState.confirmLabel).toBe('Destroy');
	});

	test('honours a custom confirm label', () => {
		void confirmDangerous('Forget tank?', 'Type "tank" to confirm', 'tank', {
			confirmLabel: 'Forget filesystem'
		});
		expect(confirmDangerousState.confirmLabel).toBe('Forget filesystem');
	});

	test('confirmDangerousRespond(true) resolves with true and closes', async () => {
		const p = confirmDangerous('x', 'y', 'z');
		confirmDangerousRespond(true);
		await expect(p).resolves.toBe(true);
		expect(confirmDangerousState.open).toBe(false);
		expect(confirmDangerousState.confirmLabel).toBe('Destroy');
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
