import { afterEach, describe, expect, test } from 'vitest';
import { confirm, confirmRespond, confirmState } from './confirm.svelte';

afterEach(() => {
	// The dialog is module-singleton state; reset between tests so a leftover
	// open=true from one test doesn't affect the next.
	confirmState.open = false;
	confirmState.resolve = null;
});

describe('confirm', () => {
	test('opens the dialog and populates title and message', () => {
		void confirm('Delete share?', 'This cannot be undone.');
		expect(confirmState.open).toBe(true);
		expect(confirmState.title).toBe('Delete share?');
		expect(confirmState.message).toBe('This cannot be undone.');
	});

	test('uses default Confirm/Cancel labels when options are omitted', () => {
		void confirm('Title');
		expect(confirmState.confirmLabel).toBe('Confirm');
		expect(confirmState.cancelLabel).toBe('Cancel');
	});

	test('honours custom labels passed via options', () => {
		void confirm('Reboot?', 'Are you sure?', {
			confirmLabel: 'Reboot now',
			cancelLabel: 'Wait'
		});
		expect(confirmState.confirmLabel).toBe('Reboot now');
		expect(confirmState.cancelLabel).toBe('Wait');
	});

	test('omitted message defaults to the empty string', () => {
		void confirm('Just title');
		expect(confirmState.message).toBe('');
	});

	test('confirmRespond(true) resolves the promise with true and closes the dialog', async () => {
		const p = confirm('x', 'y');
		confirmRespond(true);
		await expect(p).resolves.toBe(true);
		expect(confirmState.open).toBe(false);
	});

	test('confirmRespond(false) resolves with false', async () => {
		const p = confirm('x', 'y');
		confirmRespond(false);
		await expect(p).resolves.toBe(false);
	});

	test('confirmRespond clears resolve so a stray second call does not throw', () => {
		void confirm('x', 'y');
		confirmRespond(true);
		// Second response after the promise already resolved must be a no-op,
		// not an error — guards against double-click on the confirm button.
		expect(() => confirmRespond(true)).not.toThrow();
	});
});
