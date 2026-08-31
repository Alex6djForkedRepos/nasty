<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { getClient } from '$lib/client';
	import { withToast } from '$lib/toast.svelte';
	import { createIoHistory } from '$lib/history.svelte';
	import {
		dashboardPrefs,
		dashboardPresets,
		resolveDashboardDensity,
		resolveDashboardWidgets,
		type DashboardPreferences,
		type DashboardWidgetId,
		type DashboardWidgetWidth,
	} from '$lib/dashboard.svelte';
	import type {
		ActiveAlert,
		DiskHealth,
		DiskIoStats,
		Filesystem,
		FsUsage,
		NetIfStats,
		ResourceHistory,
		SystemHealth,
		SystemInfo,
		SystemStats,
		SystemStatus,
	} from '$lib/types';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent } from '$lib/components/ui/card';
	import AlertsWidget from '$lib/components/dashboard/alerts-widget.svelte';
	import CustomizeDialog from '$lib/components/dashboard/customize-dialog.svelte';
	import DiskIoWidget from '$lib/components/dashboard/disk-io-widget.svelte';
	import HistoryWidget from '$lib/components/dashboard/history-widget.svelte';
	import NetworkWidget from '$lib/components/dashboard/network-widget.svelte';
	import OperationsWidget from '$lib/components/dashboard/operations-widget.svelte';
	import StorageWidget from '$lib/components/dashboard/storage-widget.svelte';
	import SummaryWidget from '$lib/components/dashboard/summary-widget.svelte';
	import SystemWidget from '$lib/components/dashboard/system-widget.svelte';
	import { Settings2 } from '@lucide/svelte';

	type MetricsRange = '5m' | '1h' | '1d' | '7d' | '30d';
	type DiskRate = { readRate: number; writeRate: number };
	type NetworkRate = { rxRate: number; txRate: number };
	type ChartSample = { time: Date; in: number; out: number };

	const client = getClient();
	const rangeDurations: Record<MetricsRange, number> = {
		'5m': 300_000,
		'1h': 3_600_000,
		'1d': 86_400_000,
		'7d': 604_800_000,
		'30d': 2_592_000_000,
	};

	let info = $state<SystemInfo | null>(null);
	let health = $state<SystemHealth | null>(null);
	let stats = $state<SystemStats | null>(null);
	let filesystems = $state<Filesystem[]>([]);
	let alerts = $state<ActiveAlert[]>([]);
	let systemStatus = $state<SystemStatus | null>(null);
	let diskHealth = $state<DiskHealth[]>([]);
	let filesystemUsages = $state<Record<string, FsUsage>>({});
	let loading = $state(true);
	let filesystemsLoaded = $state(false);
	let infoLoaded = $state(false);
	let healthLoaded = $state(false);
	let alertsLoaded = $state(false);
	let operationsLoaded = $state(false);
	let customizeOpen = $state(false);
	let refreshTimer: ReturnType<typeof setInterval> | null = null;
	let refreshTick = 0;
	let refreshInFlight = false;
	let filesystemRequest = 0;
	let storageRequest = 0;
	let statsRequest = 0;
	let infoRequest = 0;
	let healthRequest = 0;
	let alertsRequest = 0;
	let operationsRequest = 0;

	let metricsRange = $state<MetricsRange>('5m');
	let metricsOffset = $state(0);
	let previousDiskIo = $state<DiskIoStats[]>([]);
	let previousNetworkIo = $state<NetIfStats[]>([]);
	let previousSampleTime = $state(0);
	let diskRates = $state<Map<string, DiskRate>>(new Map());
	let networkRates = $state<Map<string, NetworkRate>>(new Map());
	let networkSamples = $state<Map<string, ChartSample[]>>(new Map());
	let diskSamples = $state<Map<string, ChartSample[]>>(new Map());
	let cpuSamples = $state<ChartSample[]>([]);
	let memorySamples = $state<ChartSample[]>([]);
	let historyLoading = $state(false);
	let historyRequest = 0;

	const networkHistory = createIoHistory();
	const diskHistory = createIoHistory();
	const cpuHistory = createIoHistory();
	const memoryHistory = createIoHistory();

	let widgets = $derived(resolveDashboardWidgets(dashboardPrefs.value));
	let density = $derived(resolveDashboardDensity(dashboardPrefs.value));
	let presetLabel = $derived(
		dashboardPrefs.value.preset === 'custom'
			? 'Custom'
			: dashboardPresets[dashboardPrefs.value.preset].label
	);

	function hasWidget(id: DashboardWidgetId): boolean {
		return widgets.some((widget) => widget.id === id);
	}

	function needsStats(): boolean {
		return widgets.some((widget) => ['summary', 'storage', 'history', 'network', 'disk_io'].includes(widget.id));
	}

	function needsMetricsHistory(): boolean {
		return widgets.some((widget) => ['history', 'network', 'disk_io'].includes(widget.id));
	}

	function widgetClass(width: DashboardWidgetWidth): string {
		return width === 'full' ? 'min-w-0 xl:col-span-2' : 'min-w-0 xl:col-span-1';
	}

	function handleEvent(_: string, params: unknown) {
		const event = params as { collection?: string };
		if (event?.collection === 'filesystem') void loadFilesystemData();
	}

	onMount(() => {
		client.onEvent(handleEvent);
		void loadVisibleData(true).finally(() => loading = false);
		refreshTimer = setInterval(refreshVisibleData, 15_000);
		return () => {
			client.offEvent(handleEvent);
			if (refreshTimer) clearInterval(refreshTimer);
		};
	});

	async function loadVisibleData(showErrors: boolean) {
		const load = async () => {
			const tasks: Promise<unknown>[] = [
				fetchFilesystemInventory(),
			];
			if (hasWidget('system')) {
				tasks.push(loadSystemInfo());
				tasks.push(loadSystemHealth());
			}
			if (needsStats()) {
				const request = ++statsRequest;
				tasks.push(client.call<SystemStats>('system.stats').then((value) => {
					if (request !== statsRequest) return;
					stats = value;
					previousDiskIo = value.disk_io;
					previousNetworkIo = value.network;
					previousSampleTime = Date.now();
				}));
			}
			if (hasWidget('alerts')) tasks.push(loadAlerts());
			if (hasWidget('operations')) tasks.push(loadOperations());
			const results = await Promise.allSettled(tasks);
			if (hasWidget('storage')) await loadStorageDetails();
			if (needsMetricsHistory()) await loadMetrics();
			const failure = results.find((result): result is PromiseRejectedResult => result.status === 'rejected');
			if (failure) throw failure.reason;
		};

		if (showErrors) await withToast(load);
		else {
			try { await load(); } catch { /* Keep the last good widget data. */ }
		}
	}

	async function fetchFilesystemInventory() {
		const request = ++filesystemRequest;
		const value = await client.call<Filesystem[]>('fs.list');
		if (request !== filesystemRequest) return;
		filesystems = value;
		filesystemsLoaded = true;
	}

	async function loadSystemInfo() {
		const request = ++infoRequest;
		const value = await client.call<SystemInfo>('system.info');
		if (request !== infoRequest) return;
		info = value;
		infoLoaded = true;
	}

	async function loadSystemHealth() {
		const request = ++healthRequest;
		const value = await client.call<SystemHealth>('system.health');
		if (request !== healthRequest) return;
		health = value;
		healthLoaded = true;
	}

	async function loadAlerts() {
		const request = ++alertsRequest;
		const value = await client.call<ActiveAlert[]>('system.alerts');
		if (request !== alertsRequest) return;
		alerts = value;
		alertsLoaded = true;
	}

	async function loadOperations() {
		const request = ++operationsRequest;
		const value = await client.call<SystemStatus>('system.status');
		if (request !== operationsRequest) return;
		systemStatus = value;
		operationsLoaded = true;
	}

	async function loadFilesystemData() {
		try {
			await fetchFilesystemInventory();
			if (hasWidget('storage')) await loadStorageDetails();
		} catch { /* Keep the last good filesystem inventory. */ }
	}

	async function loadStorageDetails() {
		const request = ++storageRequest;
		const mountedFilesystems = filesystems.filter((filesystem) => filesystem.mounted);
		const [freshHealth, usageEntries] = await Promise.all([
			client.call<DiskHealth[]>('system.disks').catch(() => null),
			Promise.all(mountedFilesystems.map(async (filesystem) => {
				try {
					return [filesystem.name, await client.call<FsUsage>('fs.usage', { name: filesystem.name })] as const;
				} catch {
					return null;
				}
			})),
		]);
		if (request !== storageRequest) return;
		if (freshHealth) diskHealth = freshHealth;
		const mountedNames = new Set(mountedFilesystems.map((filesystem) => filesystem.name));
		const retainedUsages = Object.fromEntries(Object.entries(filesystemUsages).filter(([name]) => mountedNames.has(name)));
		filesystemUsages = {
			...retainedUsages,
			...Object.fromEntries(usageEntries.filter((entry): entry is readonly [string, FsUsage] => entry !== null)),
		};
	}

	async function refreshVisibleData() {
		if (refreshInFlight) return;
		refreshInFlight = true;
		refreshTick += 1;
		try {
			const tasks: Promise<unknown>[] = [];
			if (needsStats()) tasks.push(refreshStats());
			if (hasWidget('alerts')) tasks.push(loadAlerts());
			if (hasWidget('operations')) tasks.push(loadOperations());
			if (hasWidget('system') && refreshTick % 4 === 0) {
				tasks.push(loadSystemHealth());
				if (!infoLoaded) tasks.push(loadSystemInfo());
			}
			if (refreshTick % 4 === 0) tasks.push(loadFilesystemData());
			else if (hasWidget('storage') && refreshTick % 2 === 0) tasks.push(loadStorageDetails());
			await Promise.all(tasks);
		} catch {
			/* Polling is best effort. */
		} finally {
			refreshInFlight = false;
		}
	}

	async function refreshStats() {
		const request = ++statsRequest;
		const next = await client.call<SystemStats>('system.stats');
		if (request !== statsRequest) return;
		const now = Date.now();
		const elapsed = (now - previousSampleTime) / 1000;
		const sampleTime = new Date(now);

		if (previousSampleTime > 0 && elapsed > 0) {
			const nextDiskRates = new Map<string, DiskRate>();
			for (const current of next.disk_io) {
				const previous = previousDiskIo.find((device) => device.name === current.name);
				if (!previous) continue;
				const readRate = Math.max(0, (current.read_bytes - previous.read_bytes) / elapsed);
				const writeRate = Math.max(0, (current.write_bytes - previous.write_bytes) / elapsed);
				nextDiskRates.set(current.name, { readRate, writeRate });
				if (metricsRange === '5m' && metricsOffset === 0) diskHistory.push(current.name, sampleTime, readRate, writeRate);
			}
			diskRates = nextDiskRates;
			if (metricsRange === '5m' && metricsOffset === 0) {
				diskSamples = new Map(next.disk_io.map((device) => [device.name, liveSamples(diskHistory.getSamples(device.name), sampleTime)]));
			}

			const nextNetworkRates = new Map<string, NetworkRate>();
			for (const current of next.network) {
				const previous = previousNetworkIo.find((iface) => iface.name === current.name);
				if (!previous) continue;
				const rxRate = Math.max(0, (current.rx_bytes - previous.rx_bytes) / elapsed);
				const txRate = Math.max(0, (current.tx_bytes - previous.tx_bytes) / elapsed);
				nextNetworkRates.set(current.name, { rxRate, txRate });
				if (metricsRange === '5m' && metricsOffset === 0) networkHistory.push(current.name, sampleTime, rxRate, txRate);
			}
			networkRates = nextNetworkRates;
			if (metricsRange === '5m' && metricsOffset === 0) {
				networkSamples = new Map(next.network.map((iface) => [iface.name, liveSamples(networkHistory.getSamples(iface.name), sampleTime)]));
			}

			if (metricsRange === '5m' && metricsOffset === 0) {
				const cpuPercent = Math.min(100, (next.cpu.load_1 / next.cpu.count) * 100);
				const memoryPercent = next.memory.total_bytes > 0 ? next.memory.used_bytes / next.memory.total_bytes * 100 : 0;
				cpuHistory.push('cpu', sampleTime, cpuPercent, 0);
				memoryHistory.push('memory', sampleTime, memoryPercent, 0);
				cpuSamples = liveSamples(cpuHistory.getSamples('cpu'), sampleTime);
				memorySamples = liveSamples(memoryHistory.getSamples('memory'), sampleTime);
			}
		}

		previousDiskIo = next.disk_io;
		previousNetworkIo = next.network;
		previousSampleTime = now;
		stats = next;
	}

	function liveSamples(samples: ChartSample[], now: Date): ChartSample[] {
		const cutoff = now.getTime() - rangeDurations['5m'];
		return samples.filter((sample) => sample.time.getTime() >= cutoff);
	}

	function applyHistory(
		network: ResourceHistory[] | null,
		disk: ResourceHistory[] | null,
		cpu: ResourceHistory[] | null,
		memory: ResourceHistory[] | null,
	) {
		if (network) {
			networkHistory.clear();
			for (const resource of network) for (const sample of resource.samples) networkHistory.push(resource.name, new Date(sample.ts), sample.in_rate, sample.out_rate);
			networkSamples = new Map(network.map((resource) => [resource.name, [...networkHistory.getSamples(resource.name)]]));
		}
		if (disk) {
			diskHistory.clear();
			for (const resource of disk) for (const sample of resource.samples) diskHistory.push(resource.name, new Date(sample.ts), sample.in_rate, sample.out_rate);
			diskSamples = new Map(disk.map((resource) => [resource.name, [...diskHistory.getSamples(resource.name)]]));
		}
		if (cpu) {
			cpuHistory.clear();
			for (const resource of cpu) for (const sample of resource.samples) cpuHistory.push('cpu', new Date(sample.ts), sample.in_rate, 0);
			cpuSamples = [...cpuHistory.getSamples('cpu')];
		}
		if (memory) {
			memoryHistory.clear();
			for (const resource of memory) for (const sample of resource.samples) memoryHistory.push('memory', new Date(sample.ts), sample.in_rate, 0);
			memorySamples = [...memoryHistory.getSamples('memory')];
		}
	}

	async function loadMetrics(clearFailedSeries = false) {
		const request = ++historyRequest;
		historyLoading = true;
		try {
			const params = { range: metricsRange, ...(metricsOffset > 0 ? { offset: metricsOffset } : {}) };
			const [network, disk, cpu, memory] = await Promise.allSettled([
				hasWidget('network') ? client.call<ResourceHistory[]>('system.metrics.history', { kind: 'net', ...params }) : Promise.resolve([]),
				hasWidget('disk_io') ? client.call<ResourceHistory[]>('system.metrics.history', { kind: 'disk', ...params }) : Promise.resolve([]),
				hasWidget('history') ? client.call<ResourceHistory[]>('system.metrics.history', { kind: 'cpu', ...params }) : Promise.resolve([]),
				hasWidget('history') ? client.call<ResourceHistory[]>('system.metrics.history', { kind: 'mem', ...params }) : Promise.resolve([]),
			]);
			if (request === historyRequest) applyHistory(
				network.status === 'fulfilled' ? network.value : clearFailedSeries ? [] : null,
				disk.status === 'fulfilled' ? disk.value : clearFailedSeries ? [] : null,
				cpu.status === 'fulfilled' ? cpu.value : clearFailedSeries ? [] : null,
				memory.status === 'fulfilled' ? memory.value : clearFailedSeries ? [] : null,
			);
		} catch {
			/* History fills from live samples when unavailable. */
		} finally {
			if (request === historyRequest) historyLoading = false;
		}
	}

	async function changeRange(range: MetricsRange) {
		metricsRange = range;
		metricsOffset = 0;
		await loadMetrics(true);
	}

	async function navigateBack() {
		metricsOffset += rangeDurations[metricsRange];
		await loadMetrics(true);
	}

	async function navigateForward() {
		metricsOffset = Math.max(0, metricsOffset - rangeDurations[metricsRange]);
		await loadMetrics(true);
	}

	async function navigateLive() {
		metricsOffset = 0;
		await loadMetrics(true);
	}

	async function applyPreferences(preferences: DashboardPreferences) {
		dashboardPrefs.set(preferences);
		await tick();
		await loadVisibleData(false);
	}
</script>

<div class="mb-4 flex flex-wrap items-center justify-between gap-3">
	<div>
		<div class="text-sm font-semibold">{presetLabel} dashboard</div>
		<div class="text-xs text-muted-foreground">{widgets.length} visible widget{widgets.length === 1 ? '' : 's'} - {density} density</div>
	</div>
	<Button variant="outline" size="sm" onclick={() => customizeOpen = true}><Settings2 /> Customize</Button>
</div>

{#if filesystemsLoaded && filesystems.length === 0}
	<Card class="mb-4 border-primary/30 bg-primary/5">
		<CardContent class="flex flex-wrap items-center gap-6 py-6">
			<div class="min-w-0 flex-1"><h2 class="text-lg font-bold">Get started with NASty</h2><p class="mt-1 text-sm text-muted-foreground">Create your first filesystem to start storing and sharing data.</p></div>
			<Button onclick={() => goto('/filesystems?create')}>Create filesystem</Button>
		</CardContent>
	</Card>
{/if}

{#if loading}
	<Card><CardContent class="py-10 text-center text-sm text-muted-foreground">Loading dashboard...</CardContent></Card>
{:else}
	<div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
		{#each widgets as widget (widget.id)}
			<div class={widgetClass(widget.width)}>
				{#if widget.id === 'alerts'}
					<AlertsWidget {alerts} loaded={alertsLoaded} {density} />
				{:else if widget.id === 'system'}
					<SystemWidget {info} {health} loaded={infoLoaded || healthLoaded} {density} />
				{:else if widget.id === 'summary' && stats}
					<SummaryWidget {stats} {filesystems} width={widget.width} {density} />
				{:else if widget.id === 'operations'}
					<OperationsWidget operations={systemStatus?.operations ?? []} loaded={operationsLoaded} {density} />
				{:else if widget.id === 'storage'}
					<StorageWidget {filesystems} usages={filesystemUsages} health={diskHealth} rates={diskRates} {density} />
				{:else if widget.id === 'history'}
					<HistoryWidget cpuSamples={cpuSamples} memorySamples={memorySamples} range={metricsRange} offset={metricsOffset} rangeDuration={rangeDurations[metricsRange]} loading={historyLoading} width={widget.width} {density} onRange={(range) => void changeRange(range)} onBack={() => void navigateBack()} onForward={() => void navigateForward()} onLive={() => void navigateLive()} />
				{:else if widget.id === 'network' && stats}
					<NetworkWidget interfaces={stats.network} rates={networkRates} samples={networkSamples} {density} />
				{:else if widget.id === 'disk_io' && stats}
					<DiskIoWidget devices={stats.disk_io} rates={diskRates} samples={diskSamples} {density} />
				{:else}
					<Card><CardContent class="py-8 text-center text-sm text-muted-foreground">Widget data is unavailable.</CardContent></Card>
				{/if}
			</div>
		{/each}
	</div>
{/if}

<CustomizeDialog bind:open={customizeOpen} preferences={dashboardPrefs.value} onSave={(preferences) => void applyPreferences(preferences)} />
