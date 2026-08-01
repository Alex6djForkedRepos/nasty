<script lang="ts">
	import { appMonogram, resolveAppIcon } from '$lib/app-icons';
	import type { App } from '$lib/types';

	let { app, size = 32 }: { app: App; size?: number } = $props();
	const icon = $derived(resolveAppIcon(app));
	const fallback = $derived(appMonogram(app.name));
</script>

{#if icon}
	<span
		class="inline-flex shrink-0 items-center justify-center rounded-lg border border-border/70 bg-white shadow-sm"
		style={`width: ${size}px; height: ${size}px`}
		title={icon.title}
	>
		<svg
			viewBox="0 0 24 24"
			width={Math.round(size * 0.58)}
			height={Math.round(size * 0.58)}
			style={`color: #${icon.hex}`}
			aria-hidden="true"
		>
			<path fill="currentColor" d={icon.path}></path>
		</svg>
	</span>
{:else}
	<span
		class="inline-flex shrink-0 items-center justify-center rounded-lg border border-white/10 text-[0.65rem] font-bold tracking-wide text-white shadow-sm"
		style={`width: ${size}px; height: ${size}px; background-color: hsl(${fallback.hue} 52% 42%)`}
		aria-hidden="true"
	>
		{fallback.initials}
	</span>
{/if}
