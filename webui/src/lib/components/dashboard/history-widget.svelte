<script lang="ts">
	import type { DashboardDensity, DashboardWidgetWidth } from '$lib/dashboard.svelte';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import IoChart from '$lib/components/io-chart.svelte';

	type MetricsRange = '5m' | '1h' | '1d' | '7d' | '30d';
	type Sample = { time: Date; in: number; out: number };

	let {
		cpuSamples,
		memorySamples,
		range,
		offset,
		rangeDuration,
		density,
		width,
		loading,
		onRange,
		onBack,
		onForward,
		onLive,
	}: {
		cpuSamples: Sample[];
		memorySamples: Sample[];
		range: MetricsRange;
		offset: number;
		rangeDuration: number;
		density: DashboardDensity;
		width: DashboardWidgetWidth;
		loading: boolean;
		onRange: (range: MetricsRange) => void;
		onBack: () => void;
		onForward: () => void;
		onLive: () => void;
	} = $props();
</script>

<div>
	<div class="mb-3 flex flex-wrap items-center justify-between gap-2">
		<div class="flex items-center gap-2">
			<span class="text-sm font-semibold">History{loading ? ' - loading' : ''}</span>
			{#if offset > 0}
				<span class="text-xs text-muted-foreground">{new Date(Date.now() - offset - rangeDuration).toLocaleString()} - {new Date(Date.now() - offset).toLocaleString()}</span>
			{/if}
		</div>
		<div class="flex max-w-full items-center gap-1 overflow-x-auto pb-1">
			{#if offset > 0}<button type="button" onclick={onLive} disabled={loading} class="rounded-md border border-border px-2 py-1 text-xs font-medium text-primary hover:bg-accent disabled:opacity-40">Live</button>{/if}
			<button type="button" onclick={onBack} disabled={loading} class="rounded-md border border-border px-2 py-1 text-xs text-muted-foreground hover:bg-accent disabled:opacity-40" aria-label="Earlier history">&larr;</button>
			<div class="flex rounded-md border border-border">
				{#each (['5m', '1h', '1d', '7d', '30d'] as const) as option}
					<button type="button" onclick={() => onRange(option)} disabled={loading} aria-pressed={range === option} class="px-2.5 py-1 text-xs font-medium first:rounded-l-md last:rounded-r-md disabled:opacity-40 {range === option ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent hover:text-foreground'}">{option}</button>
				{/each}
			</div>
			<button type="button" onclick={onForward} disabled={offset === 0 || loading} class="rounded-md border border-border px-2 py-1 text-xs text-muted-foreground hover:bg-accent disabled:cursor-default disabled:opacity-30" aria-label="Later history">&rarr;</button>
		</div>
	</div>
	<div class="grid grid-cols-1 gap-3 {width === 'full' ? 'sm:grid-cols-2' : ''}">
		<Card>
			<CardHeader class={density === 'compact' ? 'px-4 py-2.5' : 'pb-2'}><CardTitle class="text-xs uppercase tracking-wide text-muted-foreground">CPU usage</CardTitle></CardHeader>
			<CardContent class={density === 'compact' ? 'px-4 pb-3' : ''}><IoChart samples={cpuSamples} inLabel="Usage" inColor="var(--chart-3)" yFormat={(value) => value.toFixed(0) + '%'} tooltipFormat={(value) => value.toFixed(1) + '%'} ariaLabel="CPU usage history" emptyMessage={loading ? 'Loading data...' : 'No data for selected interval.'} /></CardContent>
		</Card>
		<Card>
			<CardHeader class={density === 'compact' ? 'px-4 py-2.5' : 'pb-2'}><CardTitle class="text-xs uppercase tracking-wide text-muted-foreground">Memory usage</CardTitle></CardHeader>
			<CardContent class={density === 'compact' ? 'px-4 pb-3' : ''}><IoChart samples={memorySamples} inLabel="Used" inColor="var(--chart-5)" yFormat={(value) => value.toFixed(0) + '%'} tooltipFormat={(value) => value.toFixed(1) + '%'} ariaLabel="Memory usage history" emptyMessage={loading ? 'Loading data...' : 'No data for selected interval.'} /></CardContent>
		</Card>
	</div>
</div>
