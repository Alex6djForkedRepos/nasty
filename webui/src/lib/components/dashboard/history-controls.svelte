<script lang="ts">
	type MetricsRange = '5m' | '1h' | '1d' | '7d' | '30d';

	let {
		range,
		offset,
		loading,
		onRange,
		onBack,
		onForward,
		onLive,
		class: className = '',
	}: {
		range: MetricsRange;
		offset: number;
		loading: boolean;
		onRange: (range: MetricsRange) => void;
		onBack: () => void;
		onForward: () => void;
		onLive: () => void;
		class?: string;
	} = $props();
</script>

<div class="flex min-w-0 max-w-full items-center gap-1 overflow-x-auto {className}">
	{#if offset > 0}<button type="button" onclick={onLive} disabled={loading} class="rounded-md border border-border px-2 py-1 text-xs font-medium text-primary hover:bg-accent disabled:opacity-40">Live</button>{/if}
	<button type="button" onclick={onBack} disabled={loading} class="rounded-md border border-border px-2 py-1 text-xs text-muted-foreground hover:bg-accent disabled:opacity-40" aria-label="Earlier history">&larr;</button>
	<div class="flex rounded-md border border-border">
		{#each (['5m', '1h', '1d', '7d', '30d'] as const) as option}
			<button type="button" onclick={() => onRange(option)} disabled={loading} aria-pressed={range === option} class="px-2.5 py-1 text-xs font-medium first:rounded-l-md last:rounded-r-md disabled:opacity-40 {range === option ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent hover:text-foreground'}">{option}</button>
		{/each}
	</div>
	<button type="button" onclick={onForward} disabled={offset === 0 || loading} class="rounded-md border border-border px-2 py-1 text-xs text-muted-foreground hover:bg-accent disabled:cursor-default disabled:opacity-30" aria-label="Later history">&rarr;</button>
</div>
