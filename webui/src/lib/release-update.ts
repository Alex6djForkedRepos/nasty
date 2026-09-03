import type { UpdateInfo } from '$lib/types';

export type ReleaseUpdateRequestState = 'idle' | 'loading' | 'ready' | 'failed';
export type ReleaseUpdateKind = 'loading' | 'current' | 'available' | 'unknown' | 'failed';
export const RELEASE_UPDATE_CHANGED_EVENT = 'nasty:release-update-changed';

export interface ReleaseUpdateChangedDetail {
	info: UpdateInfo | null;
	requestState: ReleaseUpdateRequestState;
}

interface UpdateRpcClient {
	call<T>(method: string, params?: unknown, timeoutMs?: number): Promise<T>;
}

export type ReleaseUpdateCheckMode = 'fresh' | 'cached';

const updateChecksInFlight: Record<ReleaseUpdateCheckMode, Promise<UpdateInfo> | null> = {
	fresh: null,
	cached: null,
};
let releaseUpdateSnapshot: ReleaseUpdateChangedDetail | null = null;
const UPDATE_CHECK_TIMEOUT_MS = 180_000;

async function fetchCachedReleaseUpdateCheck(): Promise<UpdateInfo> {
	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), UPDATE_CHECK_TIMEOUT_MS);
	try {
		const response = await fetch('/api/v1/system/update/check_cached', { signal: controller.signal });
		if (!response.ok) throw new Error(`Update check failed with HTTP ${response.status}`);
		return await response.json() as UpdateInfo;
	} finally {
		clearTimeout(timeout);
	}
}

export interface ReleaseUpdateDisplay {
	kind: ReleaseUpdateKind;
	label: string;
	title: string;
}

export function shouldCheckReleaseUpdate(
	info: Pick<UpdateInfo, 'update_available' | 'error'>,
): boolean {
	return info.update_available === null && !info.error;
}

export function publishReleaseUpdate(
	info: UpdateInfo | null,
	requestState: ReleaseUpdateRequestState,
) {
	setReleaseUpdateSnapshot(info, requestState);
	if (typeof window === 'undefined') return;
	window.dispatchEvent(new CustomEvent<ReleaseUpdateChangedDetail>(RELEASE_UPDATE_CHANGED_EVENT, {
		detail: { info, requestState },
	}));
}

export function getReleaseUpdateSnapshot(): ReleaseUpdateChangedDetail | null {
	return releaseUpdateSnapshot;
}

export function setReleaseUpdateSnapshot(
	info: UpdateInfo | null,
	requestState: ReleaseUpdateRequestState,
) {
	releaseUpdateSnapshot = { info, requestState };
}

export function requestReleaseUpdateCheck(
	client: UpdateRpcClient,
	mode: ReleaseUpdateCheckMode = 'fresh',
): Promise<UpdateInfo> {
	if (updateChecksInFlight[mode]) return updateChecksInFlight[mode];
	const request = mode === 'cached'
		? fetchCachedReleaseUpdateCheck()
		: client.call<UpdateInfo>('system.update.check', undefined, UPDATE_CHECK_TIMEOUT_MS);
	updateChecksInFlight[mode] = request;
	const clear = () => {
		if (updateChecksInFlight[mode] === request) updateChecksInFlight[mode] = null;
	};
	request.then(clear, clear);
	return request;
}

export function invalidateReleaseUpdateCheck() {
	updateChecksInFlight.fresh = null;
	updateChecksInFlight.cached = null;
}

export function releaseUpdateDisplay(
	info: Pick<UpdateInfo, 'current_version' | 'latest_version' | 'update_available' | 'error'> | null,
	requestState: ReleaseUpdateRequestState,
): ReleaseUpdateDisplay {
	if (requestState === 'loading') {
		return { kind: 'loading', label: 'checking', title: 'Checking for NASty updates.' };
	}
	if (requestState === 'failed') {
		return { kind: 'failed', label: 'check failed', title: 'NASty update status could not be loaded.' };
	}
	if (info?.error) {
		return { kind: 'failed', label: 'check failed', title: `NASty update check failed: ${info.error}` };
	}
	if (info?.update_available === true) {
		return {
			kind: 'available',
			label: 'update',
			title: info.latest_version
				? `NASty update ${info.latest_version} is available. Open the Update page.`
				: 'A NASty update is available. Open the Update page.',
		};
	}
	if (info?.update_available === false) {
		return {
			kind: 'current',
			label: 'current',
			title: `NASty ${info.current_version} is current.`,
		};
	}
	return {
		kind: 'unknown',
		label: 'unchecked',
		title: 'NASty update status has not been checked.',
	};
}
