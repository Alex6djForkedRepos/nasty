<script lang="ts">
	import { onMount, tick } from 'svelte';
	import {
		dashboardWidgetSupportsNarrowWidth,
		dashboardWidgetSupportsTiny,
		dashboardWidgetMeta,
		type DashboardWidgetConfig,
		type DashboardWidgetPresentation,
		type DashboardWidgetWidth,
	} from '$lib/dashboard.svelte';
	import { Check, Ellipsis } from '@lucide/svelte';

	let { widget, onChange }: {
		widget: DashboardWidgetConfig;
		onChange: (patch: Partial<Pick<DashboardWidgetConfig, 'width' | 'presentation'>>) => void;
	} = $props();

	let open = $state(false);
	let root: HTMLDivElement;
	let trigger: HTMLButtonElement;
	let panel: HTMLDivElement;
	let panelPosition = $state({ left: 8, top: 8, width: 176 });
	const widths: { value: DashboardWidgetWidth; label: string }[] = [
		{ value: 'full', label: 'Full width' },
		{ value: 'half', label: '1/2 width' },
		{ value: 'third', label: '1/3 width' },
		{ value: 'quarter', label: '1/4 width' },
	];

	let availableWidths = $derived(widths.filter(({ value }) =>
		(value === 'full' || value === 'half')
		|| dashboardWidgetSupportsNarrowWidth(widget.id, widget.presentation)
	));

	onMount(() => {
		const closeOutside = (event: Event) => {
			if (!root.contains(event.target as Node)) open = false;
		};
		const closeOnViewportChange = () => {
			if (!open) return;
			open = false;
			trigger.focus();
		};
		document.addEventListener('pointerdown', closeOutside);
		document.addEventListener('focusin', closeOutside);
		window.addEventListener('resize', closeOnViewportChange);
		window.addEventListener('scroll', closeOnViewportChange, true);
		return () => {
			document.removeEventListener('pointerdown', closeOutside);
			document.removeEventListener('focusin', closeOutside);
			window.removeEventListener('resize', closeOnViewportChange);
			window.removeEventListener('scroll', closeOnViewportChange, true);
		};
	});

	async function toggle() {
		open = !open;
		if (!open) return;
		await tick();
		const triggerRect = trigger.getBoundingClientRect();
		const width = Math.min(176, Math.max(0, window.innerWidth - 16));
		const height = panel.getBoundingClientRect().height;
		const below = triggerRect.bottom + 4;
		panelPosition = {
			left: Math.max(8, Math.min(triggerRect.right - width, window.innerWidth - width - 8)),
			top: below + height <= window.innerHeight - 8 ? below : Math.max(8, triggerRect.top - height - 4),
			width,
		};
		panel.querySelector<HTMLButtonElement>('button')?.focus();
	}

	function select(patch: Partial<Pick<DashboardWidgetConfig, 'width' | 'presentation'>>) {
		onChange(patch);
		open = false;
		trigger.focus();
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape' || !open) return;
		event.stopPropagation();
		open = false;
		trigger.focus();
	}

	function selectPresentation(presentation: DashboardWidgetPresentation) {
		select({ presentation });
	}
</script>

<div bind:this={root} class="relative">
	<button bind:this={trigger} type="button" onclick={() => void toggle()} onkeydown={handleKeydown} aria-label={`Configure ${dashboardWidgetMeta[widget.id].label} widget`} aria-haspopup="dialog" aria-expanded={open} class="rounded p-1.5 hover:bg-accent hover:text-foreground">
		<Ellipsis class="h-3.5 w-3.5" />
	</button>
	<div bind:this={panel} hidden={!open} role="dialog" aria-label="Widget display options" tabindex="-1" onkeydown={handleKeydown} style={`left: ${panelPosition.left}px; top: ${panelPosition.top}px; width: ${panelPosition.width}px;`} class="fixed z-40 rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-lg">
		<div class="px-2 pb-1 pt-1.5 text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground">Width</div>
		{#each availableWidths as option}
			<button type="button" aria-pressed={widget.width === option.value} onclick={() => select({ width: option.value })} class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent">
				<span class="w-3.5">{#if widget.width === option.value}<Check class="h-3.5 w-3.5" />{/if}</span>{option.label}
			</button>
		{/each}
		{#if dashboardWidgetSupportsTiny(widget.id)}
			<div class="my-1 border-t border-border"></div>
			<div class="px-2 pb-1 pt-1.5 text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground">Presentation</div>
			{#each ([{ value: 'standard', label: 'Standard' }, { value: 'tiny', label: 'Tiny' }] as const) as option}
				<button type="button" aria-pressed={widget.presentation === option.value} onclick={() => selectPresentation(option.value)} class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-xs hover:bg-accent">
					<span class="w-3.5">{#if widget.presentation === option.value}<Check class="h-3.5 w-3.5" />{/if}</span>{option.label}
				</button>
			{/each}
		{/if}
	</div>
</div>
