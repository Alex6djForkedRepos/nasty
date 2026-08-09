/**
 * Dangerous-action confirm dialog — requires the user to type a confirmation string.
 * Mount <ConfirmDangerousDialog /> once in the root layout, then call:
 *   if (!await confirmDangerous('Delete X?', 'Type "X" to confirm', 'X')) return;
 */

import { registerSessionReset } from './client';

interface ConfirmDangerousState {
	open: boolean;
	title: string;
	message: string;
	expectedValue: string;
	confirmLabel: string;
	resolve: ((v: boolean) => void) | null;
}

interface ConfirmDangerousOptions {
	confirmLabel?: string;
}

export const confirmDangerousState = $state<ConfirmDangerousState>({
	open: false,
	title: '',
	message: '',
	expectedValue: '',
	confirmLabel: 'Destroy',
	resolve: null,
});

export function confirmDangerous(
	title: string,
	message: string,
	expectedValue: string,
	options?: ConfirmDangerousOptions,
): Promise<boolean> {
	return new Promise((resolve) => {
		confirmDangerousState.resolve?.(false);
		confirmDangerousState.title = title;
		confirmDangerousState.message = message;
		confirmDangerousState.expectedValue = expectedValue;
		confirmDangerousState.confirmLabel = options?.confirmLabel ?? 'Destroy';
		confirmDangerousState.resolve = resolve;
		confirmDangerousState.open = true;
	});
}

export function confirmDangerousRespond(value: boolean) {
	confirmDangerousState.open = false;
	confirmDangerousState.resolve?.(value);
	confirmDangerousState.resolve = null;
	confirmDangerousState.title = '';
	confirmDangerousState.message = '';
	confirmDangerousState.expectedValue = '';
	confirmDangerousState.confirmLabel = 'Destroy';
}

registerSessionReset(() => confirmDangerousRespond(false));
