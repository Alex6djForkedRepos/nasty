<script lang="ts">
	import type { DashboardDensity, DashboardWidgetPresentation, DashboardWidgetWidth } from '$lib/dashboard.svelte';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import IoChart from '$lib/components/io-chart.svelte';

	type MetricsRange = '5m' | '1h' | '1d' | '7d' | '30d';
	type Sample = { time: Date; in: number; out: number };

	let {
		cpuSamples,
		memorySamples,
		range,
		density,
		width,
		loading,
		presentation,
	}: {
		cpuSamples: Sample[];
		memorySamples: Sample[];
		range: MetricsRange;
		density: DashboardDensity;
		width: DashboardWidgetWidth;
		loading: boolean;
		presentation: DashboardWidgetPresentation;
	} = $props();

	function summarize(samples: Sample[]): { current: number; peak: number } | null {
		if (samples.length === 0) return null;
		return {
			current: samples.at(-1)!.in,
			peak: Math.max(...samples.map((sample) => sample.in)),
		};
	}

	function boundedPercent(value: number): number {
		return Math.min(100, Math.max(0, value));
	}

	function utilizationColor(percent: number): string {
		if (percent > 90) return 'var(--color-red-500)';
		if (percent > 75) return 'var(--color-amber-500)';
		return 'var(--color-emerald-500)';
	}

	let cpu = $derived(summarize(cpuSamples));
	let memory = $derived(summarize(memorySamples));
	let memoryColor = $derived(utilizationColor(memory?.current ?? 0));
</script>

{#if presentation === 'tiny'}
	<div class="grid grid-cols-2 gap-3 {width === 'third' || width === 'quarter' ? 'xl:grid-cols-1' : ''}">
		<Card class="gap-0 py-0">
			<CardContent class="px-4 py-3">
				<CardTitle class="text-xs uppercase tracking-wide text-muted-foreground">CPU usage</CardTitle>
				{#if cpu}
					<div class="mt-1 text-2xl font-bold tabular-nums">{cpu.current.toFixed(1)}%</div>
					<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-secondary" aria-hidden="true"><div class="h-full rounded-full bg-[var(--chart-3)]" style="width: {boundedPercent(cpu.current)}%"></div></div>
					<div class="mt-1.5 text-xs text-muted-foreground">{range} peak {cpu.peak.toFixed(1)}%</div>
				{:else}
					<div class="mt-2 text-sm font-medium text-muted-foreground" role="status">{loading ? 'Loading...' : 'Unavailable'}</div>
				{/if}
			</CardContent>
		</Card>
		<Card class="gap-0 py-0">
			<CardContent class="px-4 py-3">
				<CardTitle class="text-xs uppercase tracking-wide text-muted-foreground">Memory usage</CardTitle>
				{#if memory}
					<div class="mt-1 text-2xl font-bold tabular-nums">{memory.current.toFixed(1)}%</div>
					<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-secondary" aria-hidden="true"><div class="h-full rounded-full" style="width: {boundedPercent(memory.current)}%; background-color: {memoryColor}"></div></div>
					<div class="mt-1.5 text-xs text-muted-foreground">{range} peak {memory.peak.toFixed(1)}%</div>
				{:else}
					<div class="mt-2 text-sm font-medium text-muted-foreground" role="status">{loading ? 'Loading...' : 'Unavailable'}</div>
				{/if}
			</CardContent>
		</Card>
	</div>
{:else}
	<div class="grid grid-cols-1 gap-3 {width === 'full' ? 'sm:grid-cols-2' : ''}">
		<Card>
			<CardHeader class={density === 'compact' ? 'px-4 py-2.5' : 'pb-2'}><CardTitle class="text-xs uppercase tracking-wide text-muted-foreground">CPU usage</CardTitle></CardHeader>
			<CardContent class={density === 'compact' ? 'px-4 pb-3' : ''}><IoChart samples={cpuSamples} inLabel="Usage" inColor="var(--chart-3)" yFormat={(value) => value.toFixed(0) + '%'} tooltipFormat={(value) => value.toFixed(1) + '%'} ariaLabel="CPU usage history" emptyMessage={loading ? 'Loading data...' : 'No data for selected interval.'} /></CardContent>
		</Card>
		<Card>
			<CardHeader class={density === 'compact' ? 'px-4 py-2.5' : 'pb-2'}><CardTitle class="text-xs uppercase tracking-wide text-muted-foreground">Memory usage</CardTitle></CardHeader>
			<CardContent class={density === 'compact' ? 'px-4 pb-3' : ''}><IoChart samples={memorySamples} inLabel="Used" inColor={memoryColor} yFormat={(value) => value.toFixed(0) + '%'} tooltipFormat={(value) => value.toFixed(1) + '%'} ariaLabel="Memory usage history" emptyMessage={loading ? 'Loading data...' : 'No data for selected interval.'} /></CardContent>
		</Card>
	</div>
{/if}
