<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import type { DashboardDensity } from '$lib/dashboard.svelte';
	import type { DashboardHealthFreshness } from '$lib/dashboard-health';
	import type { BackupScheduleEntry } from '$lib/types';
	import { Card, CardContent } from '$lib/components/ui/card';
	import { CalendarClock } from '@lucide/svelte';

	let {
		entries = null,
		currentTime = null,
		freshness,
		density,
	}: {
		entries?: BackupScheduleEntry[] | null;
		currentTime?: string | null;
		freshness: DashboardHealthFreshness;
		density: DashboardDensity;
	} = $props();

	function hostOffset(timestamp: string | null): number | null {
		if (!timestamp) return null;
		const parsed = Date.parse(timestamp);
		return Number.isFinite(parsed) ? parsed - Date.now() : null;
	}

	let now = $state(new Date());
	let serverOffset = $state<number | null>(hostOffset(untrack(() => currentTime)));
	let hostNow = $derived(serverOffset === null ? null : new Date(now.getTime() + serverOffset));
	let visibleEntries = $derived((entries ?? []).slice(0, density === 'compact' ? 3 : 4));

	$effect(() => {
		serverOffset = hostOffset(currentTime);
		now = new Date();
	});

	onMount(() => {
		const timer = window.setInterval(() => now = new Date(), 30_000);
		return () => window.clearInterval(timer);
	});

	function datePart(timestamp: string, part: 'day' | 'month'): string {
		return new Intl.DateTimeFormat(undefined, {
			day: part === 'day' ? '2-digit' : undefined,
			month: part === 'month' ? 'short' : undefined,
			timeZone: 'UTC',
		}).format(new Date(timestamp));
	}

	function timeLabel(timestamp: string): string {
		return new Intl.DateTimeFormat(undefined, {
			weekday: 'short',
			hour: '2-digit',
			minute: '2-digit',
			hourCycle: 'h23',
			timeZone: 'UTC',
		}).format(new Date(timestamp));
	}

	function relativeLabel(timestamp: string): string | null {
		if (!hostNow) return null;
		const minutes = Math.max(0, Math.ceil((new Date(timestamp).getTime() - hostNow.getTime()) / 60_000));
		if (minutes < 1) return 'due now';
		if (minutes < 60) return `in ${minutes}m`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `in ${hours}h ${minutes % 60}m`;
		const days = Math.floor(hours / 24);
		return `in ${days}d ${hours % 24}h`;
	}
</script>

<Card class="h-full overflow-hidden">
	<CardContent class={density === 'compact' ? 'p-3' : 'p-4'}>
		<div class="mb-3 flex items-center justify-between gap-3">
			<div class="flex items-center gap-2">
				<CalendarClock class="h-4 w-4 text-primary" />
				<span class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Upcoming backups</span>
			</div>
			<div class="flex items-center gap-2 text-[0.68rem] font-medium text-muted-foreground">
				{#if freshness === 'refreshing'}<span>Refreshing</span>{/if}
				{#if freshness === 'stale'}<span class="text-amber-500" role="status">Stale</span>{/if}
				<span>Next cron - UTC</span>
			</div>
		</div>

		{#if freshness === 'loading'}
			<p class="text-sm text-muted-foreground">Loading backup schedule...</p>
		{:else if freshness === 'unavailable' || entries === null}
			<p class="text-sm text-muted-foreground">Backup schedule could not be loaded.</p>
		{:else if entries.length === 0}
			<div class="rounded-md border border-dashed border-border px-3 py-4 text-center">
				<p class="text-sm font-medium">No scheduled backups</p>
				<a href="/backups" class="mt-1 inline-block text-xs text-primary hover:underline">Manage backup profiles</a>
			</div>
		{:else}
			<div class="divide-y divide-border">
				{#each visibleEntries as entry}
					{@const relative = entry.next_run_at ? relativeLabel(entry.next_run_at) : null}
					<a href="/backups" class="flex items-center gap-3 py-2 first:pt-0 last:pb-0 outline-none hover:text-primary focus-visible:ring-2 focus-visible:ring-ring" title={entry.schedule_error ?? entry.schedule}>
						{#if entry.next_run_at}
							<div class="w-11 shrink-0 rounded-md border border-border bg-secondary/30 py-1 text-center leading-none">
								<div class="text-[0.6rem] font-semibold uppercase text-muted-foreground">{datePart(entry.next_run_at, 'month')}</div>
								<div class="mt-1 text-lg font-bold tabular-nums">{datePart(entry.next_run_at, 'day')}</div>
							</div>
						{:else}
							<div class="flex h-11 w-11 shrink-0 items-center justify-center rounded-md border border-amber-500/50 bg-amber-500/10 text-lg font-bold text-amber-500">!</div>
						{/if}
						<div class="min-w-0 flex-1">
							<div class="truncate text-sm font-semibold">{entry.profile_name}</div>
							{#if entry.next_run_at}
								<div class="mt-0.5 text-xs text-muted-foreground">{timeLabel(entry.next_run_at)}{relative ? ` - ${relative}` : ''}</div>
							{:else}
								<div class="mt-0.5 truncate text-xs text-amber-500">{entry.schedule_error ? 'Invalid schedule' : 'No future runs'}</div>
							{/if}
						</div>
						{#if entry.last_run}
							<span class="h-2 w-2 shrink-0 rounded-full {entry.last_run.success ? 'bg-emerald-500' : 'bg-red-500'}" title={`Last run ${entry.last_run.success ? 'succeeded' : 'failed'}`}>
								<span class="sr-only">Last run {entry.last_run.success ? 'succeeded' : 'failed'}</span>
							</span>
						{/if}
					</a>
				{/each}
			</div>
			{#if entries.length > visibleEntries.length}
				<a href="/backups" class="mt-3 block text-right text-xs text-primary hover:underline">{entries.length - visibleEntries.length} more scheduled</a>
			{/if}
		{/if}
	</CardContent>
</Card>
