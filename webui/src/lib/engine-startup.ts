import type { BootPhase, BootStatus } from '$lib/types';

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
type RetryWait = (milliseconds: number, signal: AbortSignal) => Promise<boolean>;

const OVERALL_STATES = new Set(['booting', 'ready', 'ready_with_errors']);
const PHASE_STATES = new Set(['pending', 'running', 'ok', 'failed']);

function isNullableNumber(value: unknown): value is number | null {
	return value === null || typeof value === 'number';
}

function isBootPhase(value: unknown): value is BootPhase {
	if (!value || typeof value !== 'object') return false;
	const phase = value as Record<string, unknown>;
	return typeof phase.name === 'string'
		&& typeof phase.state === 'string'
		&& PHASE_STATES.has(phase.state)
		&& isNullableNumber(phase.started_at_ms)
		&& isNullableNumber(phase.finished_at_ms)
		&& isNullableNumber(phase.duration_ms)
		&& (phase.error === null || typeof phase.error === 'string');
}

export function isBootStatus(value: unknown): value is BootStatus {
	if (!value || typeof value !== 'object') return false;
	const status = value as Record<string, unknown>;
	return typeof status.overall === 'string'
		&& OVERALL_STATES.has(status.overall)
		&& Array.isArray(status.phases)
		&& status.phases.every(isBootPhase)
		&& typeof status.process_started_at_unix === 'number'
		&& isNullableNumber(status.ready_at_ms);
}

export function waitingBootStatus(now = Date.now()): BootStatus {
	return {
		overall: 'booting',
		phases: [],
		process_started_at_unix: Math.floor(now / 1000),
		ready_at_ms: null,
	};
}

export async function fetchBootStatus(
	fetcher: Fetcher = fetch,
	signal?: AbortSignal,
): Promise<BootStatus | null> {
	try {
		const response = await fetcher('/api/boot_status', {
			cache: 'no-store',
			signal,
		});
		if (!response.ok) return null;
		const body: unknown = await response.json();
		return isBootStatus(body) ? body : null;
	} catch {
		return null;
	}
}

async function waitForRetry(milliseconds: number, signal: AbortSignal): Promise<boolean> {
	if (signal.aborted) return false;
	return new Promise((resolve) => {
		const timeout = setTimeout(() => {
			signal.removeEventListener('abort', abort);
			resolve(true);
		}, milliseconds);
		const abort = () => {
			clearTimeout(timeout);
			resolve(false);
		};
		signal.addEventListener('abort', abort, { once: true });
	});
}

export async function pollEngineUntilReady(options: {
	fetcher?: Fetcher;
	onStatus: (status: BootStatus) => void;
	signal: AbortSignal;
	retryMs?: number;
	wait?: RetryWait;
}): Promise<BootStatus | null> {
	const {
		fetcher = fetch,
		onStatus,
		signal,
		retryMs = 600,
		wait = waitForRetry,
	} = options;

	while (!signal.aborted) {
		const status = await fetchBootStatus(fetcher, signal);
		if (signal.aborted) return null;
		onStatus(status ?? waitingBootStatus());
		if (status && status.overall !== 'booting') return status;
		if (!await wait(retryMs, signal)) return null;
	}
	return null;
}
