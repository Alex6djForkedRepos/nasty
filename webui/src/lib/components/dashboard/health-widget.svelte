<script lang="ts">
	import type { DashboardDensity } from '$lib/dashboard.svelte';
	import type {
		DashboardHealthFreshness,
		ManagedContainerHealthSummary,
		ServiceHealthSummary,
	} from '$lib/dashboard-health';
	import { Card, CardContent } from '$lib/components/ui/card';

	let {
		kind,
		services = null,
		containers = null,
		freshness,
		density,
	}: {
		kind: 'services' | 'containers';
		services?: ServiceHealthSummary | null;
		containers?: ManagedContainerHealthSummary | null;
		freshness: DashboardHealthFreshness;
		density: DashboardDensity;
	} = $props();

	type TilePresentation = {
		label: string;
		detail: string;
		tone: 'healthy' | 'warning' | 'neutral';
	};

	function stalePresentation(previous: TilePresentation): TilePresentation {
		return {
			label: `Stale - ${previous.label.toLowerCase()}`,
			detail: `Refresh failed; showing last known ${previous.label.toLowerCase()} state.`,
			tone: 'warning',
		};
	}

	function refreshingPresentation(previous: TilePresentation): TilePresentation {
		return {
			label: `Refreshing - ${previous.label.toLowerCase()}`,
			detail: `Refreshing; showing last known ${previous.label.toLowerCase()} state.`,
			tone: 'neutral',
		};
	}

	function servicePresentation(): TilePresentation {
		if (freshness === 'loading') return { label: 'Loading', detail: 'Checking enabled services.', tone: 'neutral' };
		if (!services || freshness === 'unavailable') return { label: 'Unavailable', detail: 'Service health could not be loaded.', tone: 'neutral' };
		const current: TilePresentation = services.enabled === 0
			? { label: 'No services enabled', detail: 'No services are expected to run.', tone: 'neutral' }
			: services.running === services.enabled
				? { label: 'Healthy', detail: 'All enabled services are running.', tone: 'healthy' }
				: { label: 'Unhealthy', detail: `${services.enabled - services.running} enabled service${services.enabled - services.running === 1 ? '' : 's'} not running.`, tone: 'warning' };
		if (freshness === 'refreshing') return refreshingPresentation(current);
		return freshness === 'stale' ? stalePresentation(current) : current;
	}

	function containerPresentation(): TilePresentation {
		if (freshness === 'loading') return { label: 'Loading', detail: 'Checking managed containers.', tone: 'neutral' };
		if (!containers || freshness === 'unavailable') return { label: 'Unavailable', detail: 'Managed container health could not be loaded.', tone: 'neutral' };
		let current: TilePresentation;
		if (containers.runtime === 'disabled') {
			current = { label: 'Disabled', detail: 'Docker runtime is disabled.', tone: 'neutral' };
		} else if (containers.runtime === 'down') {
			current = { label: 'Unhealthy', detail: `Docker runtime is enabled but not running.${containers.expected == null ? ' Expected count unavailable.' : ''}`, tone: 'warning' };
		} else if (containers.expected == null || containers.running == null) {
			current = { label: 'Unavailable', detail: 'Expected container inventory is unavailable.', tone: 'neutral' };
		} else if (containers.expected === 0) {
			current = { label: 'Healthy', detail: 'No managed containers are configured.', tone: 'healthy' };
		} else if (containers.running === containers.expected) {
			current = { label: 'Healthy', detail: 'All expected containers are running.', tone: 'healthy' };
		} else {
			const stopped = containers.expected - containers.running;
			current = { label: 'Unhealthy', detail: `${stopped} expected container${stopped === 1 ? '' : 's'} not running.`, tone: 'warning' };
		}
		if (freshness === 'refreshing') return refreshingPresentation(current);
		return freshness === 'stale' ? stalePresentation(current) : current;
	}

	function tileClass(tone: TilePresentation['tone']): string {
		if (tone === 'healthy') return 'border-emerald-500/30 bg-emerald-500/5';
		if (tone === 'warning') return 'border-amber-500/40 bg-amber-500/5';
		return 'border-border';
	}

	function labelClass(tone: TilePresentation['tone']): string {
		if (tone === 'healthy') return 'text-emerald-500';
		if (tone === 'warning') return 'text-amber-500';
		return 'text-muted-foreground';
	}

	let state = $derived(kind === 'services' ? servicePresentation() : containerPresentation());
</script>

<Card class="h-full overflow-hidden {tileClass(state.tone)}">
	{#if kind === 'services'}
		<a href="/services" class="block h-full rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background" aria-label={`Services: ${state.label}. ${services ? `${services.enabled} enabled, ${services.running} running.` : state.detail}`}>
			<CardContent class={density === 'compact' ? 'px-4 py-3' : 'px-5 py-4'}>
				<div class="flex items-center justify-between gap-3">
					<div class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Services</div>
					<div class="text-xs font-semibold {labelClass(state.tone)}">{state.label}</div>
				</div>
				{#if services}
					<div class="mt-2 text-xl font-bold tabular-nums">{services.enabled} enabled <span class="text-muted-foreground">/</span> {services.running} running</div>
				{:else}
					<div class="mt-2 text-xl font-bold text-muted-foreground">{state.label}</div>
				{/if}
				<div class="mt-1 text-xs text-muted-foreground" role={freshness === 'refreshing' || freshness === 'stale' || freshness === 'unavailable' ? 'status' : undefined}>{state.detail}</div>
			</CardContent>
		</a>
	{:else}
		<a href="/apps" class="block h-full rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background" aria-label={`Managed containers: ${state.label}. ${containers?.expected != null ? `${containers.expected} expected, ${containers.running} running. Simple apps count once; Compose service instances count individually.` : state.detail}`}>
			<CardContent class={density === 'compact' ? 'px-4 py-3' : 'px-5 py-4'}>
				<div class="flex items-center justify-between gap-3">
					<div class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Managed containers</div>
					<div class="text-xs font-semibold {labelClass(state.tone)}">{state.label}</div>
				</div>
				{#if containers?.expected != null && containers.running != null}
					<div class="mt-2 text-xl font-bold tabular-nums">{containers.expected} expected <span class="text-muted-foreground">/</span> {containers.running} running</div>
				{:else}
					<div class="mt-2 text-xl font-bold text-muted-foreground">{state.label}</div>
				{/if}
				<div class="mt-1 text-xs text-muted-foreground" role={freshness === 'refreshing' || freshness === 'stale' || freshness === 'unavailable' ? 'status' : undefined}>{state.detail}</div>
			</CardContent>
		</a>
	{/if}
</Card>
