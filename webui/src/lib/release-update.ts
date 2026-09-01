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

let updateCheckInFlight: Promise<UpdateInfo> | null = null;
const UPDATE_CHECK_TIMEOUT_MS = 180_000;

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
	if (typeof window === 'undefined') return;
	window.dispatchEvent(new CustomEvent<ReleaseUpdateChangedDetail>(RELEASE_UPDATE_CHANGED_EVENT, {
		detail: { info, requestState },
	}));
}

export function requestReleaseUpdateCheck(client: UpdateRpcClient): Promise<UpdateInfo> {
	if (updateCheckInFlight) return updateCheckInFlight;
	const request = client.call<UpdateInfo>('system.update.check', undefined, UPDATE_CHECK_TIMEOUT_MS);
	updateCheckInFlight = request;
	const clear = () => {
		if (updateCheckInFlight === request) updateCheckInFlight = null;
	};
	request.then(clear, clear);
	return request;
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
