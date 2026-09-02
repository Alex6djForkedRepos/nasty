<script lang="ts">
	import type { DashboardDensity } from '$lib/dashboard.svelte';
	import type { DashboardHealthFreshness, ManagedContainerHealthSummary } from '$lib/dashboard-health';
	import type { AppsStatus, VmStatus } from '$lib/types';
	import { formatBytes } from '$lib/format';
	import { Card, CardContent } from '$lib/components/ui/card';

	let {
		vms = null,
		vmFreshness,
		appsStatus = null,
		containers = null,
		containerFreshness,
		density,
	}: {
		vms?: VmStatus[] | null;
		vmFreshness: DashboardHealthFreshness;
		appsStatus?: AppsStatus | null;
		containers?: ManagedContainerHealthSummary | null;
		containerFreshness: DashboardHealthFreshness;
		density: DashboardDensity;
	} = $props();

	let runningVms = $derived(vms?.filter((vm) => vm.running) ?? []);
	let activeVcpus = $derived(runningVms.reduce((total, vm) => total + vm.cpus, 0));
	let activeMemory = $derived(runningVms.reduce((total, vm) => total + vm.memory_mib, 0) * 1024 * 1024);

	function freshnessLabel(freshness: DashboardHealthFreshness): string | null {
		if (freshness === 'loading') return 'Loading';
		if (freshness === 'refreshing') return 'Refreshing';
		if (freshness === 'stale') return 'Stale';
		if (freshness === 'unavailable') return 'Unavailable';
		return null;
	}

	function freshnessClass(freshness: DashboardHealthFreshness): string {
		return freshness === 'stale' ? 'text-amber-500' : 'text-muted-foreground';
	}

	function freshnessDescription(freshness: DashboardHealthFreshness): string {
		return freshness === 'current' ? '' : ` Data ${freshness}.`;
	}

	function vmAriaLabel(): string {
		if (!vms) return vmFreshness === 'loading' ? 'Virtual machine status loading.' : 'Virtual machine status unavailable.';
		return `Virtual machines: ${runningVms.length} of ${vms.length} running. ${activeVcpus} active vCPU and ${formatBytes(activeMemory)} active memory.${freshnessDescription(vmFreshness)}`;
	}

	function dockerLabel(): string {
		if (!appsStatus) return containerFreshness === 'loading' ? 'Loading' : 'Unavailable';
		if (!appsStatus.enabled) return 'Disabled';
		if (!appsStatus.running) return 'Runtime down';
		if (containers?.expected == null || containers.running == null) return 'Inventory unavailable';
		return `${containers.running} / ${containers.expected}`;
	}

	function dockerDetail(): string {
		if (!appsStatus) return containerFreshness === 'loading' ? 'Checking Docker runtime.' : 'Docker status could not be loaded.';
		if (!appsStatus.enabled) return 'Docker runtime is disabled.';
		if (!appsStatus.running) return 'Docker is enabled but not running.';
		if (containers?.expected == null || containers.running == null) return `${appsStatus.app_count} configured app${appsStatus.app_count === 1 ? '' : 's'}.`;
		return `${appsStatus.app_count} app${appsStatus.app_count === 1 ? '' : 's'} - containers running`;
	}

	function dockerClass(): string {
		if (containerFreshness === 'stale') return 'text-amber-500';
		if (!appsStatus || !appsStatus.enabled) return 'text-muted-foreground';
		if (!appsStatus.running) return 'text-amber-500';
		if (containers?.expected != null && containers.running === containers.expected) return 'text-emerald-500';
		return 'text-amber-500';
	}
</script>

<Card class="h-full overflow-hidden">
	<CardContent class="p-0">
		<div class="grid grid-cols-2 divide-x divide-border">
			<a href="/vms" class="min-w-0 {density === 'compact' ? 'p-3' : 'p-4'} outline-none transition-colors hover:bg-accent/40 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring" aria-label={vmAriaLabel()}>
				<div class="flex min-w-0 items-center justify-between gap-2">
					<div class="truncate text-xs font-medium uppercase tracking-wide text-muted-foreground">Virtual machines</div>
					{#if freshnessLabel(vmFreshness)}<div class="shrink-0 text-[0.68rem] font-semibold {freshnessClass(vmFreshness)}" role={vmFreshness === 'refreshing' || vmFreshness === 'stale' || vmFreshness === 'unavailable' ? 'status' : undefined}>{freshnessLabel(vmFreshness)}</div>{/if}
				</div>
				{#if vms}
					<div class="mt-2 text-2xl font-bold tabular-nums">{runningVms.length} <span class="text-base font-medium text-muted-foreground">/ {vms.length}</span></div>
					<div class="text-xs text-muted-foreground">running</div>
					<div class="mt-2 truncate text-xs text-muted-foreground" title={`${activeVcpus} active vCPU - ${formatBytes(activeMemory)} active memory`}>{runningVms.length > 0 ? `${activeVcpus} vCPU - ${formatBytes(activeMemory)} active` : vms.length > 0 ? 'No active VM allocation' : 'No VMs defined'}</div>
				{:else}
					<div class="mt-2 text-lg font-bold text-muted-foreground">{vmFreshness === 'loading' ? 'Loading' : 'Unavailable'}</div>
					<div class="mt-2 text-xs text-muted-foreground">{vmFreshness === 'loading' ? 'Checking VM inventory.' : 'VM inventory could not be loaded.'}</div>
				{/if}
			</a>

			<a href="/apps" class="min-w-0 {density === 'compact' ? 'p-3' : 'p-4'} outline-none transition-colors hover:bg-accent/40 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring" aria-label={`Docker apps: ${dockerLabel()}. ${dockerDetail()}${freshnessDescription(containerFreshness)}`}>
				<div class="flex min-w-0 items-center justify-between gap-2">
					<div class="truncate text-xs font-medium uppercase tracking-wide text-muted-foreground">Docker apps</div>
					{#if freshnessLabel(containerFreshness)}<div class="shrink-0 text-[0.68rem] font-semibold {freshnessClass(containerFreshness)}" role={containerFreshness === 'refreshing' || containerFreshness === 'stale' || containerFreshness === 'unavailable' ? 'status' : undefined}>{freshnessLabel(containerFreshness)}</div>{/if}
				</div>
				<div class="mt-2 truncate text-2xl font-bold tabular-nums {dockerClass()}" title={dockerLabel()}>{dockerLabel()}</div>
				<div class="mt-2 text-xs text-muted-foreground">{dockerDetail()}</div>
			</a>
		</div>
	</CardContent>
</Card>
