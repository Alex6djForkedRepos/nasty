<script lang="ts">
	import type { DashboardDensity } from '$lib/dashboard.svelte';
	import type { Filesystem, SystemStats } from '$lib/types';
	import { formatBytes, formatPercent } from '$lib/format';
	import { formatTemp } from '$lib/temperature.svelte';
	import { Card, CardContent } from '$lib/components/ui/card';

	let { kind, stats = null, filesystems = [], filesystemsLoaded = true, density }: {
		kind: 'cpu_load' | 'memory_usage' | 'cpu_status' | 'storage_summary';
		stats?: SystemStats | null;
		filesystems?: Filesystem[];
		filesystemsLoaded?: boolean;
		density: DashboardDensity;
	} = $props();

	function cpuPercent(): number {
		return stats ? Math.min(100, (stats.cpu.load_1 / stats.cpu.count) * 100) : 0;
	}

	function memoryPercent(): number {
		return stats && stats.memory.total_bytes > 0 ? (stats.memory.used_bytes / stats.memory.total_bytes) * 100 : 0;
	}

	function totalStorage(): { used: number; total: number; unknown: number } {
		return filesystems.reduce((total, filesystem) => {
			const capacity = Math.max(0, filesystem.used_bytes + filesystem.available_bytes);
			const known = filesystem.mounted && capacity > 0;
			return {
				used: total.used + (known ? Math.max(0, filesystem.used_bytes) : 0),
				total: total.total + (known ? capacity : 0),
				unknown: total.unknown + (known ? 0 : 1),
			};
		}, { used: 0, total: 0, unknown: 0 });
	}

	function barColor(percent: number): string {
		if (percent > 90) return 'bg-red-500';
		if (percent > 75) return 'bg-amber-500';
		return 'bg-primary';
	}

	let storage = $derived(totalStorage());
</script>

<Card class="h-full">
	<CardContent class={density === 'compact' ? 'px-4 py-3' : 'pt-4 pb-3'}>
		{#if kind === 'cpu_load'}
			<div class="text-xs uppercase tracking-wide text-muted-foreground">CPU load</div>
			{#if stats}
				<div class="mt-1 flex items-baseline gap-2"><span class="text-2xl font-bold">{stats.cpu.load_1.toFixed(2)}</span><span class="text-xs text-muted-foreground">/ {stats.cpu.count} cores</span></div>
				<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-secondary"><div class="h-full rounded-full {barColor(cpuPercent())}" style="width: {cpuPercent()}%"></div></div>
				<div class="mt-1.5 flex justify-between text-xs tabular-nums text-muted-foreground"><span>5m {stats.cpu.load_5.toFixed(2)}</span><span>15m {stats.cpu.load_15.toFixed(2)}</span></div>
			{:else}
				<div class="mt-1 text-2xl font-bold text-muted-foreground">Unavailable</div>
			{/if}
		{:else if kind === 'memory_usage'}
			<div class="text-xs uppercase tracking-wide text-muted-foreground">Memory</div>
			{#if stats}
				<div class="mt-1 flex flex-wrap items-baseline gap-x-2"><span class="text-2xl font-bold">{formatPercent(stats.memory.used_bytes, stats.memory.total_bytes)}</span><span class="text-xs text-muted-foreground">{formatBytes(stats.memory.used_bytes)} / {formatBytes(stats.memory.total_bytes)}</span></div>
				<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-secondary"><div class="h-full rounded-full {barColor(memoryPercent())}" style="width: {memoryPercent()}%"></div></div>
				{#if stats.memory.bcachefs_btree_cache_bytes != null}<div class="mt-1.5 truncate text-xs text-muted-foreground" title="Approximate kernel-reported bcachefs btree-node buffers">Btree cache {formatBytes(stats.memory.bcachefs_btree_cache_bytes)}</div>{/if}
			{:else}
				<div class="mt-1 text-2xl font-bold text-muted-foreground">Unavailable</div>
			{/if}
		{:else if kind === 'cpu_status'}
			<div class="text-xs uppercase tracking-wide text-muted-foreground">CPU</div>
			{#if stats && (stats.cpu.temp_c != null || stats.cpu.freq_mhz != null)}
				<div class="mt-1 text-2xl font-bold">{formatTemp(stats.cpu.temp_c) ?? '-'}</div>
				<div class="mt-2 text-xs text-muted-foreground">{stats.cpu.freq_mhz != null ? (stats.cpu.freq_mhz >= 1000 ? (stats.cpu.freq_mhz / 1000).toFixed(1) + ' GHz' : stats.cpu.freq_mhz + ' MHz') : ''}{stats.cpu.governor ? ` - ${stats.cpu.governor}` : ''}</div>
			{:else}
				<div class="mt-1 text-2xl font-bold text-muted-foreground">Unavailable</div>
				<div class="mt-2 text-xs text-muted-foreground">CPU temperature and frequency are unavailable.</div>
			{/if}
		{:else}
			<div class="text-xs uppercase tracking-wide text-muted-foreground">Storage</div>
			{#if !filesystemsLoaded}
				<div class="mt-1 text-2xl font-bold text-muted-foreground">Unavailable</div>
				<div class="mt-2 text-xs text-muted-foreground">Filesystem inventory could not be loaded.</div>
			{:else if filesystems.length > 0}
				{#if storage.total > 0}
					<div class="mt-1 flex flex-wrap items-baseline gap-x-2"><span class="text-2xl font-bold">{formatPercent(storage.used, storage.total)}</span><span class="text-xs text-muted-foreground">{formatBytes(storage.used)} / {formatBytes(storage.total)}</span></div>
					<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-secondary"><div class="h-full rounded-full {barColor(storage.used / storage.total * 100)}" style="width: {storage.used / storage.total * 100}%"></div></div>
				{:else}
					<div class="mt-1 text-2xl font-bold text-muted-foreground">Unavailable</div>
				{/if}
				<div class="mt-1.5 text-xs text-muted-foreground">{filesystems.length} filesystem{filesystems.length === 1 ? '' : 's'}{storage.unknown > 0 ? ` - ${storage.unknown} unavailable` : ''}</div>
			{:else}
				<div class="mt-1 text-2xl font-bold text-muted-foreground">No filesystems</div>
				<div class="mt-2 text-xs text-muted-foreground">Storage capacity will appear after a filesystem is created.</div>
			{/if}
		{/if}
	</CardContent>
</Card>
