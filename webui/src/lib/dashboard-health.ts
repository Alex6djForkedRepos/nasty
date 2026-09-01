import type { App, AppsStatus, ProtocolStatus } from '$lib/types';

export type DashboardHealthFreshness = 'loading' | 'refreshing' | 'current' | 'stale' | 'unavailable';

export interface ServiceHealthSummary {
	enabled: number;
	running: number;
}

export interface ManagedContainerHealthSummary {
	runtime: 'disabled' | 'running' | 'down';
	expected: number | null;
	running: number | null;
}

export function summarizeEnabledServices(protocols: ProtocolStatus[]): ServiceHealthSummary {
	const enabled = protocols.filter((protocol) => protocol.enabled);
	return {
		enabled: enabled.length,
		running: enabled.filter((protocol) => protocol.running).length,
	};
}

export function summarizeManagedContainers(status: AppsStatus, apps: App[]): ManagedContainerHealthSummary {
	if (!status.enabled) return { runtime: 'disabled', expected: null, running: null };

	let expected = 0;
	let running = 0;
	for (const app of apps) {
		const expectedContainers = app.expected_containers ?? (app.kind === 'simple' ? [app.name] : null);
		if (!expectedContainers) {
			return { runtime: status.running ? 'running' : 'down', expected: null, running: null };
		}
		expected += expectedContainers.length;
		if (!status.running) continue;
		if (app.kind !== 'compose') {
			if (app.status === 'running') running += 1;
			continue;
		}
		const runningByService = new Map<string, number>();
		for (const container of app.containers ?? []) {
			if (container.status !== 'running') continue;
			runningByService.set(container.name, (runningByService.get(container.name) ?? 0) + 1);
		}
		for (const name of expectedContainers) {
			const available = runningByService.get(name) ?? 0;
			if (available === 0) continue;
			running += 1;
			runningByService.set(name, available - 1);
		}
	}

	return {
		runtime: status.running ? 'running' : 'down',
		expected,
		running: status.running ? running : 0,
	};
}

export function shouldPollDashboardHealth(widgetVisible: boolean, documentHidden: boolean): boolean {
	return widgetVisible && !documentHidden;
}
