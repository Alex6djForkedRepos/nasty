<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import type { DashboardDensity } from '$lib/dashboard.svelte';
	import type { Settings, SystemInfo } from '$lib/types';
	import { Card, CardContent } from '$lib/components/ui/card';

	let {
		info = null,
		settings = null,
		loaded,
		density,
	}: {
		info?: SystemInfo | null;
		settings?: Settings | null;
		loaded: boolean;
		density: DashboardDensity;
	} = $props();

	function hostOffset(currentTime: string | undefined): number | null {
		if (!currentTime) return null;
		const timestamp = Date.parse(currentTime);
		return Number.isFinite(timestamp) ? timestamp - Date.now() : null;
	}

	let now = $state(new Date());
	let serverOffset = $state<number | null>(hostOffset(untrack(() => info?.current_time)));
	let timezone = $derived(settings?.timezone ?? info?.timezone ?? 'UTC');
	let hostNow = $derived(serverOffset === null ? null : new Date(now.getTime() + serverOffset));
	let clockParts = $derived(hostNow ? getClockParts(hostNow, timezone) : { hour: 0, minute: 0, second: 0 });
	let hourAngle = $derived((clockParts.hour % 12 + clockParts.minute / 60) * 30);
	let minuteAngle = $derived((clockParts.minute + clockParts.second / 60) * 6);
	let secondAngle = $derived(clockParts.second * 6);
	let formattedTime = $derived(hostNow ? new Intl.DateTimeFormat(undefined, {
		hour: 'numeric',
		minute: '2-digit',
		second: '2-digit',
		hour12: !(settings?.clock_24h ?? true),
		timeZone: timezone,
	}).format(hostNow) : '');
	let formattedDate = $derived(hostNow ? new Intl.DateTimeFormat(undefined, {
		weekday: 'long',
		month: 'long',
		day: 'numeric',
		year: 'numeric',
		timeZone: timezone,
	}).format(hostNow) : '');

	$effect(() => {
		serverOffset = hostOffset(info?.current_time);
		now = new Date();
	});

	onMount(() => {
		const timer = window.setInterval(() => now = new Date(), 1_000);
		return () => window.clearInterval(timer);
	});

	function getClockParts(date: Date, timeZone: string) {
		const parts = new Intl.DateTimeFormat('en-US', {
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit',
			hourCycle: 'h23',
			timeZone,
		}).formatToParts(date);
		const value = (type: Intl.DateTimeFormatPartTypes) => Number(parts.find((part) => part.type === type)?.value ?? 0);
		return { hour: value('hour'), minute: value('minute'), second: value('second') };
	}
</script>

<Card class="h-full gap-0 overflow-hidden py-0">
	<CardContent class={density === 'compact' ? 'p-3' : 'p-4'}>
		{#if hostNow}
			<div class="flex items-center gap-4">
				<svg viewBox="0 0 100 100" class="h-24 w-24 shrink-0 text-foreground" role="img" aria-label={`Analog clock showing ${formattedTime}`}>
					<circle cx="50" cy="50" r="46" class="fill-secondary/30 stroke-border" stroke-width="2" />
					{#each Array.from({ length: 12 }) as _, index}
						<line x1="50" y1="7" x2="50" y2={index % 3 === 0 ? 14 : 11} transform={`rotate(${index * 30} 50 50)`} class="stroke-muted-foreground" stroke-width={index % 3 === 0 ? 2 : 1} stroke-linecap="round" />
					{/each}
					<line x1="50" y1="50" x2="50" y2="28" transform={`rotate(${hourAngle} 50 50)`} class="stroke-foreground" stroke-width="4" stroke-linecap="round" />
					<line x1="50" y1="50" x2="50" y2="19" transform={`rotate(${minuteAngle} 50 50)`} class="stroke-foreground" stroke-width="2.5" stroke-linecap="round" />
					<line x1="50" y1="55" x2="50" y2="15" transform={`rotate(${secondAngle} 50 50)`} class="stroke-primary" stroke-width="1.25" stroke-linecap="round" />
					<circle cx="50" cy="50" r="3" class="fill-primary" />
				</svg>
				<div class="min-w-0">
					<div class="truncate text-2xl font-bold tabular-nums" title={formattedTime}>{formattedTime}</div>
					<div class="mt-1 text-sm text-muted-foreground">{formattedDate}</div>
					<div class="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
						<span class="font-mono">{timezone}</span>
						<span class="flex items-center gap-1.5">
							<span class="h-1.5 w-1.5 rounded-full {info?.ntp_synced ? 'bg-emerald-500' : 'bg-amber-500'}"></span>
							{info?.ntp_synced ? 'NTP synchronized' : 'NTP not synchronized'}
						</span>
					</div>
				</div>
			</div>
		{:else}
			<div class="flex min-h-24 items-center justify-center text-sm text-muted-foreground" role="status">
				{loaded ? 'Host time is unavailable.' : 'Loading host time...'}
			</div>
		{/if}
		{#if settings?.dashboard_motd}
			<div class="mt-4 break-words whitespace-pre-line border-t border-border pt-3 text-sm leading-relaxed">{settings.dashboard_motd}</div>
		{/if}
	</CardContent>
</Card>
