<script lang="ts">
	import type { DashboardDensity, DashboardWidgetPresentation } from '$lib/dashboard.svelte';
	import type { SystemHealth, SystemInfo } from '$lib/types';
	import { formatBytes, formatUptime } from '$lib/format';
	import { Card, CardContent } from '$lib/components/ui/card';
	import { ChevronDown, ChevronRight } from '@lucide/svelte';

	let {
		info,
		health,
		infoLoaded,
		healthLoaded,
		density,
		presentation,
	}: { info: SystemInfo | null; health: SystemHealth | null; infoLoaded: boolean; healthLoaded: boolean; density: DashboardDensity; presentation: DashboardWidgetPresentation } = $props();
	let expanded = $state(false);
	let downCount = $derived(health?.services.filter((service) => !service.running).length ?? 0);
	let loaded = $derived(infoLoaded || healthLoaded);
</script>

{#if presentation === 'tiny'}
	<Card>
		<CardContent class="flex min-h-10 flex-wrap items-center gap-x-4 gap-y-1 px-3 py-2 text-xs">
			{#if !loaded}
				<span class="font-medium text-muted-foreground">System status unavailable</span>
			{:else}
				{#if info}
					<span class="min-w-0 max-w-full truncate font-semibold" title={info.hostname}>{info.hostname}</span>
					<span class="max-w-full truncate text-muted-foreground" title={`Version ${info.version}`}>v{info.version}</span>
					<span class="text-muted-foreground">Up {formatUptime(info.uptime_seconds)}</span>
				{:else if !infoLoaded}
					<span class="font-medium text-muted-foreground">System info unavailable</span>
				{/if}
				{#if health}
					<span class="ml-auto flex items-center gap-1.5 font-semibold {health.status === 'ok' && downCount === 0 ? 'text-emerald-400' : 'text-red-400'}">
						<span class="h-1.5 w-1.5 rounded-full {health.status === 'ok' && downCount === 0 ? 'bg-emerald-400' : 'bg-red-400'}"></span>
						{downCount > 0 ? `${downCount} service${downCount === 1 ? '' : 's'} down` : health.status.toUpperCase()}
					</span>
				{:else if !healthLoaded}
					<span class="ml-auto font-medium text-muted-foreground">Health unavailable</span>
				{/if}
			{/if}
		</CardContent>
	</Card>
{:else}
	<Card>
		<CardContent class={density === 'compact' ? 'py-3' : 'py-4'}>
			{#if !loaded}
				<p class="text-sm text-muted-foreground">System status is unavailable.</p>
			{/if}
			<div class="flex flex-wrap items-center gap-x-8 gap-y-2">
				{#if info}
					<div class="flex items-center gap-2">
						<span class="text-lg font-bold">{info.hostname}</span>
						<span class="text-xs text-muted-foreground">v{info.version}</span>
					</div>
					<div class="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
						<span>Kernel {info.kernel}</span>
						<span>Up {formatUptime(info.uptime_seconds)}</span>
					</div>
				{/if}
				{#if health}
					<button
						type="button"
						onclick={() => expanded = !expanded}
						class="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-3 transition-opacity hover:opacity-80"
						aria-expanded={expanded}
					>
						<span class="text-sm font-semibold {health.status === 'ok' ? 'text-emerald-400' : 'text-red-400'}">
							{health.status.toUpperCase()}
						</span>
						{#each health.services as service}
							<span class="flex items-center gap-1.5 text-xs text-muted-foreground">
								<span class="h-1.5 w-1.5 rounded-full {service.running ? 'bg-emerald-400' : 'bg-red-400'}"></span>
								{service.name}<span class="sr-only">{service.running ? 'running' : 'down'}</span>
							</span>
						{/each}
						{#if expanded}<ChevronDown class="h-4 w-4" />{:else}<ChevronRight class="h-4 w-4" />{/if}
					</button>
				{/if}
			</div>

			{#if expanded && health}
				<div class="mt-4 grid grid-cols-1 gap-3 border-t border-border pt-4 sm:grid-cols-2">
					{#each health.services as service}
						<div class="rounded-md border border-border p-3">
							<div class="mb-2 flex items-center justify-between">
								<div class="flex items-center gap-2">
									<span class="h-2 w-2 rounded-full {service.running ? 'bg-emerald-400' : 'bg-red-400'}"></span>
									<span class="text-sm font-medium">{service.name}</span>
								</div>
								<span class="rounded-md border px-2 py-0.5 text-xs font-medium {service.running ? 'border-emerald-700 bg-emerald-950 text-emerald-400' : 'border-red-700 bg-red-950 text-red-400'}">
									{service.running ? 'Running' : 'Down'}
								</span>
							</div>
							{#if service.running && service.pid}
								<div class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
									<span class="text-muted-foreground">PID</span><span class="text-right font-mono">{service.pid}</span>
									<span class="text-muted-foreground">Memory</span><span class="text-right font-mono">{service.memory_bytes != null ? formatBytes(service.memory_bytes) : '-'}</span>
									<span class="text-muted-foreground">CPU Time</span><span class="text-right font-mono">{service.cpu_seconds != null ? service.cpu_seconds.toFixed(1) + 's' : '-'}</span>
									<span class="text-muted-foreground">Uptime</span><span class="text-right font-mono">{service.uptime_seconds != null ? formatUptime(service.uptime_seconds) : '-'}</span>
								</div>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</CardContent>
	</Card>
{/if}
