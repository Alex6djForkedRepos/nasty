<script lang="ts">
	import {
		dashboardPresets,
		dashboardWidgetMeta,
		defaultDashboardPreferences,
		type DashboardPreferences,
		type DashboardPreset,
	} from '$lib/dashboard.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { ArrowDown, ArrowUp, LayoutDashboard, RotateCcw } from '@lucide/svelte';

	let { open = $bindable(false), preferences, onSave }: {
		open: boolean;
		preferences: DashboardPreferences;
		onSave: (preferences: DashboardPreferences) => void;
	} = $props();

	let draft = $state<DashboardPreferences>(defaultDashboardPreferences());
	let wasOpen = false;

	$effect(() => {
		if (open && !wasOpen) draft = clone(preferences);
		wasOpen = open;
	});

	function clone(value: DashboardPreferences): DashboardPreferences {
		return { ...value, widgets: value.widgets.map((widget) => ({ ...widget })) };
	}

	function selectPreset(preset: DashboardPreset) {
		draft = { ...draft, preset };
	}

	function updateWidget(index: number, patch: Partial<DashboardPreferences['widgets'][number]>) {
		draft = {
			...draft,
			widgets: draft.widgets.map((widget, position) => position === index ? { ...widget, ...patch } : widget),
		};
	}

	function moveWidget(index: number, direction: -1 | 1) {
		const target = index + direction;
		if (target < 0 || target >= draft.widgets.length) return;
		const widgets = [...draft.widgets];
		[widgets[index], widgets[target]] = [widgets[target], widgets[index]];
		draft = { ...draft, widgets };
	}

	function resetCustom() {
		draft = { ...defaultDashboardPreferences(), preset: 'custom' };
	}

	function save() {
		onSave(clone(draft));
		open = false;
	}

	let canSave = $derived(draft.preset !== 'custom' || draft.widgets.some((widget) => widget.visible));
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="flex max-h-[85vh] w-[94vw] max-w-3xl flex-col gap-0 overflow-hidden p-0">
		<Dialog.Header class="border-b border-border px-6 py-5">
			<Dialog.Title class="flex items-center gap-2"><LayoutDashboard class="h-5 w-5" /> Customize dashboard</Dialog.Title>
			<Dialog.Description>Choose a focused preset or build a responsive dashboard from predefined widgets.</Dialog.Description>
		</Dialog.Header>

		<div class="overflow-y-auto px-6 py-5">
			<div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
				{#each (Object.entries(dashboardPresets) as [Exclude<DashboardPreset, 'custom'>, (typeof dashboardPresets)[Exclude<DashboardPreset, 'custom'>]][]) as [id, preset]}
					<button type="button" onclick={() => selectPreset(id)} aria-pressed={draft.preset === id} class="rounded-lg border p-3 text-left transition-colors {draft.preset === id ? 'border-primary bg-primary/10' : 'border-border hover:bg-accent'}">
						<div class="text-sm font-semibold">{preset.label}</div>
						<p class="mt-1 text-xs leading-relaxed text-muted-foreground">{preset.description}</p>
					</button>
				{/each}
				<button type="button" onclick={() => selectPreset('custom')} aria-pressed={draft.preset === 'custom'} class="rounded-lg border p-3 text-left transition-colors {draft.preset === 'custom' ? 'border-primary bg-primary/10' : 'border-border hover:bg-accent'}">
					<div class="text-sm font-semibold">Custom</div>
					<p class="mt-1 text-xs leading-relaxed text-muted-foreground">Choose visibility, order, width, and density.</p>
				</button>
			</div>

			{#if draft.preset === 'custom'}
				<div class="mt-6 flex flex-wrap items-center justify-between gap-3 border-t border-border pt-5">
					<div><h3 class="text-sm font-semibold">Custom widgets</h3><p class="text-xs text-muted-foreground">Half-width widgets share a row on wide screens and stack on mobile.</p></div>
					<div class="flex items-center gap-2">
						<label for="dashboard-density" class="text-xs font-medium text-muted-foreground">Density</label>
						<select id="dashboard-density" value={draft.density} onchange={(event) => draft = { ...draft, density: (event.currentTarget as HTMLSelectElement).value === 'compact' ? 'compact' : 'comfortable' }} class="h-8 rounded-md border border-border bg-background px-2 text-xs">
							<option value="comfortable">Comfortable</option><option value="compact">Compact</option>
						</select>
					</div>
				</div>

				<div class="mt-3 divide-y divide-border rounded-lg border border-border">
					{#each draft.widgets as widget, index (widget.id)}
						<div class="flex items-center gap-3 px-3 py-2.5">
							<input type="checkbox" checked={widget.visible} onchange={(event) => updateWidget(index, { visible: (event.currentTarget as HTMLInputElement).checked })} aria-label={`Show ${dashboardWidgetMeta[widget.id].label}`} class="h-4 w-4 accent-primary" />
							<div class="min-w-0 flex-1"><div class="text-sm font-medium">{dashboardWidgetMeta[widget.id].label}</div><div class="truncate text-xs text-muted-foreground">{dashboardWidgetMeta[widget.id].description}</div></div>
							<select value={widget.width} disabled={!widget.visible} onchange={(event) => updateWidget(index, { width: (event.currentTarget as HTMLSelectElement).value === 'half' ? 'half' : 'full' })} aria-label={`${dashboardWidgetMeta[widget.id].label} width`} class="h-8 rounded-md border border-border bg-background px-2 text-xs disabled:opacity-40"><option value="full">Full</option><option value="half">Half</option></select>
							<div class="flex">
								<Button variant="ghost" size="icon-sm" disabled={index === 0} onclick={() => moveWidget(index, -1)} aria-label={`Move ${dashboardWidgetMeta[widget.id].label} up`}><ArrowUp /></Button>
								<Button variant="ghost" size="icon-sm" disabled={index === draft.widgets.length - 1} onclick={() => moveWidget(index, 1)} aria-label={`Move ${dashboardWidgetMeta[widget.id].label} down`}><ArrowDown /></Button>
							</div>
						</div>
					{/each}
				</div>
				{#if !canSave}<p class="mt-2 text-xs text-red-400">Select at least one widget.</p>{/if}
			{/if}
		</div>

		<Dialog.Footer class="border-t border-border px-6 py-4">
			{#if draft.preset === 'custom'}<Button variant="ghost" onclick={resetCustom} class="mr-auto"><RotateCcw /> Reset custom</Button>{/if}
			<Button variant="outline" onclick={() => open = false}>Cancel</Button>
			<Button disabled={!canSave} onclick={save}>Apply dashboard</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
