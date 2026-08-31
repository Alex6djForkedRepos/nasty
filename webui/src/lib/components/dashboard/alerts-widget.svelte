<script lang="ts">
	import type { DashboardDensity } from '$lib/dashboard.svelte';
	import type { ActiveAlert } from '$lib/types';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { CheckCircle2, TriangleAlert } from '@lucide/svelte';

	let { alerts, loaded, density }: { alerts: ActiveAlert[]; loaded: boolean; density: DashboardDensity } = $props();
</script>

<Card aria-live="polite" class={alerts.some((alert) => alert.severity === 'critical') ? 'border-red-800/80' : ''}>
	<CardHeader class={density === 'compact' ? 'px-4 py-2.5' : 'pb-2'}>
		<CardTitle class="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
			{#if !loaded}
				<TriangleAlert class="h-4 w-4 text-muted-foreground" /> Alert status unavailable
			{:else if alerts.length === 0}
				<CheckCircle2 class="h-4 w-4 text-emerald-400" /> No active alerts
			{:else}
				<TriangleAlert class="h-4 w-4 text-amber-400" /> {alerts.length} active alert{alerts.length === 1 ? '' : 's'}
			{/if}
		</CardTitle>
	</CardHeader>
	{#if loaded && alerts.length > 0}
		<CardContent class={density === 'compact' ? 'px-4 pb-3' : ''}>
			<div class="space-y-2">
				{#each alerts as alert}
					<div class="flex items-start gap-3 rounded-md border px-3 py-2 text-sm {alert.severity === 'critical' ? 'border-red-800 bg-red-950/70 text-red-200' : 'border-amber-800 bg-amber-950/60 text-amber-200'}">
						<span class="mt-1 h-2 w-2 shrink-0 rounded-full {alert.severity === 'critical' ? 'bg-red-400' : 'bg-amber-400'}"></span>
						<span class="text-[0.65rem] font-bold uppercase tracking-wide">{alert.severity}</span>
						<span class="min-w-0 flex-1">{alert.message}</span>
					</div>
				{/each}
			</div>
		</CardContent>
	{/if}
</Card>
