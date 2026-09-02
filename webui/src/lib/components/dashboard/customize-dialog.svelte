<script lang="ts">
	import { tick } from 'svelte';
	import {
		DASHBOARD_VIEW_NAME_MAX_LENGTH,
		createDashboardView,
		dashboardOptionalPresets,
		dashboardPresets,
		dashboardViewNameAvailable,
		dashboardWidgetMeta,
		dashboardWidgetSupportsNarrowWidth,
		dashboardWidgetSupportsTiny,
		defaultDashboardPreferences,
		deleteDashboardView,
		getActiveDashboardView,
		renameDashboardView,
		selectDashboardView,
		setDashboardPresetTabVisible,
		updateActiveDashboardView,
		type DashboardPreferences,
		type DashboardOptionalPreset,
		type DashboardPreset,
		type DashboardWidgetWidth,
	} from '$lib/dashboard.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { ArrowDown, ArrowUp, Copy, LayoutDashboard, Pencil, Plus, RotateCcw, Trash2 } from '@lucide/svelte';

	type NameAction = 'create' | 'duplicate' | 'rename';

	let { open = $bindable(false), preferences, onSave }: {
		open: boolean;
		preferences: DashboardPreferences;
		onSave: (preferences: DashboardPreferences) => void;
	} = $props();

	let draft = $state<DashboardPreferences>(defaultDashboardPreferences());
	let nameAction = $state<NameAction | null>(null);
	let nameDraft = $state('');
	let deletePending = $state(false);
	let nameInput = $state<HTMLInputElement>();
	let nameActionButton = $state<HTMLElement>();
	let deleteButton = $state<HTMLElement>();
	let viewSelect = $state<HTMLSelectElement>();
	let wasOpen = false;

	$effect(() => {
		if (open && !wasOpen) {
			draft = clone(preferences);
			cancelNameAction();
			deletePending = false;
			deleteButton = undefined;
		}
		wasOpen = open;
	});

	let activeView = $derived(getActiveDashboardView(draft));
	let nameAvailable = $derived(dashboardViewNameAvailable(
		draft,
		nameDraft,
		nameAction === 'rename' ? activeView.id : undefined,
	));
	let invalidView = $derived(draft.customViews.find((view) => !view.widgets.some((widget) => widget.visible)));
	let canSave = $derived(!invalidView);

	function clone(value: DashboardPreferences): DashboardPreferences {
		return {
			...value,
			hiddenPresetTabs: [...value.hiddenPresetTabs],
			customViews: value.customViews.map((view) => ({
				...view,
				widgets: view.widgets.map((widget) => ({ ...widget })),
			})),
		};
	}

	function selectPreset(preset: DashboardPreset) {
		const preferences = dashboardOptionalPresets.includes(preset as DashboardOptionalPreset)
			? setDashboardPresetTabVisible(draft, preset as DashboardOptionalPreset, true)
			: draft;
		draft = {
			...preferences,
			preset,
		};
		cancelNameAction();
		deletePending = false;
	}

	function setPresetTabVisible(preset: DashboardOptionalPreset, visible: boolean) {
		draft = setDashboardPresetTabVisible(draft, preset, visible);
	}

	function selectView(id: string) {
		draft = selectDashboardView(draft, id);
		cancelNameAction();
		deletePending = false;
	}

	function updateWidget(index: number, patch: Partial<(typeof activeView.widgets)[number]>) {
		const widgets = activeView.widgets.map((widget, position) =>
			position === index ? { ...widget, ...patch } : widget
		);
		draft = updateActiveDashboardView(draft, { widgets });
	}

	function updatePresentation(index: number, value: string) {
		const widget = activeView.widgets[index];
		const presentation = value === 'tiny' && dashboardWidgetSupportsTiny(widget.id) ? 'tiny' : 'standard';
		updateWidget(index, {
			presentation,
			width: (widget.width === 'quarter' || widget.width === 'third') && !dashboardWidgetSupportsNarrowWidth(widget.id, presentation)
				? 'half'
				: widget.width,
		});
	}

	function updateDensity(value: string) {
		draft = updateActiveDashboardView(draft, {
			density: value === 'compact' ? 'compact' : 'comfortable',
		});
	}

	function parseWidgetWidth(value: string): DashboardWidgetWidth {
		return value === 'quarter' || value === 'third' || value === 'half' ? value : 'full';
	}

	function moveWidget(index: number, direction: -1 | 1) {
		const target = index + direction;
		if (target < 0 || target >= activeView.widgets.length) return;
		const widgets = activeView.widgets.map((widget) => ({ ...widget }));
		[widgets[index], widgets[target]] = [widgets[target], widgets[index]];
		draft = updateActiveDashboardView(draft, { widgets });
	}

	function resetCustom() {
		const defaults = getActiveDashboardView(defaultDashboardPreferences());
		draft = updateActiveDashboardView(draft, {
			density: defaults.density,
			widgets: defaults.widgets,
		});
	}

	function suggestedCopyName(): string {
		const base = `${activeView.name} copy`.slice(0, DASHBOARD_VIEW_NAME_MAX_LENGTH);
		if (dashboardViewNameAvailable(draft, base)) return base;
		let index = 2;
		let candidate = '';
		do {
			const suffix = ` ${index}`;
			candidate = `${base.slice(0, DASHBOARD_VIEW_NAME_MAX_LENGTH - suffix.length)}${suffix}`;
			index += 1;
		} while (!dashboardViewNameAvailable(draft, candidate));
		return candidate;
	}

	async function beginNameAction(action: NameAction, button: HTMLElement) {
		deletePending = false;
		deleteButton = undefined;
		nameActionButton = button;
		nameAction = action;
		nameDraft = action === 'rename' ? activeView.name : action === 'duplicate' ? suggestedCopyName() : '';
		await tick();
		nameInput?.focus();
	}

	function cancelNameAction(restoreFocus = false) {
		const focusTarget = nameActionButton;
		nameAction = null;
		nameDraft = '';
		nameActionButton = undefined;
		if (restoreFocus && focusTarget) requestAnimationFrame(() => focusTarget.focus());
	}

	function submitNameAction() {
		if (!nameAction || !nameAvailable) return;
		if (nameAction === 'rename') {
			draft = renameDashboardView(draft, activeView.id, nameDraft);
		} else {
			draft = createDashboardView(draft, nameDraft, nameAction === 'duplicate' ? activeView : undefined);
		}
		cancelNameAction(true);
	}

	function handleNameKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') submitNameAction();
		if (event.key === 'Escape') cancelNameAction(true);
	}

	function removeActiveView() {
		if (draft.customViews.length <= 1) return;
		draft = deleteDashboardView(draft, activeView.id);
		cancelNameAction();
		deletePending = false;
		deleteButton = undefined;
		requestAnimationFrame(() => viewSelect?.focus());
	}

	function beginDelete(button: HTMLElement) {
		cancelNameAction();
		deleteButton = button;
		deletePending = true;
	}

	function cancelDelete() {
		const focusTarget = deleteButton;
		deletePending = false;
		deleteButton = undefined;
		requestAnimationFrame(() => focusTarget?.focus());
	}

	function save() {
		onSave(clone(draft));
		open = false;
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="flex max-h-[85vh] w-[94vw] flex-col gap-0 overflow-hidden p-0 sm:max-w-5xl">
		<Dialog.Header class="border-b border-border px-6 py-5">
			<Dialog.Title class="flex items-center gap-2"><LayoutDashboard class="h-5 w-5" /> Customize dashboard</Dialog.Title>
			<Dialog.Description>Choose a focused preset or manage named Custom views built from predefined widgets.</Dialog.Description>
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
					<p class="mt-1 text-xs leading-relaxed text-muted-foreground">{draft.customViews.length} named view{draft.customViews.length === 1 ? '' : 's'}.</p>
				</button>
			</div>
			<div class="mt-4 rounded-lg border border-border bg-muted/20 px-4 py-3">
				<div class="text-sm font-semibold">Preset tabs</div>
				<p class="mt-0.5 text-xs text-muted-foreground">Choose which optional built-in views appear above the dashboard. Overview always stays visible.</p>
				<div class="mt-3 flex flex-wrap gap-x-5 gap-y-2">
					{#each dashboardOptionalPresets as preset}
						<label class="flex items-center gap-2 text-sm"><input type="checkbox" checked={!draft.hiddenPresetTabs.includes(preset)} onchange={(event) => setPresetTabVisible(preset, event.currentTarget.checked)} class="h-4 w-4 accent-primary" />{dashboardPresets[preset].label}</label>
					{/each}
				</div>
			</div>

			{#if draft.preset === 'custom'}
				<div class="mt-6 border-t border-border pt-5">
					<div class="flex flex-wrap items-end gap-2">
						<div class="min-w-48 flex-1">
							<label for="dashboard-custom-view" class="mb-1 block text-xs font-medium text-muted-foreground">Custom view</label>
							<select bind:this={viewSelect} id="dashboard-custom-view" value={activeView.id} onchange={(event) => selectView(event.currentTarget.value)} class="h-9 w-full rounded-md border border-border bg-background px-3 text-sm">
								{#each draft.customViews as view (view.id)}<option value={view.id}>{view.name}</option>{/each}
							</select>
						</div>
						<Button variant="outline" size="sm" onclick={(event) => void beginNameAction('create', event.currentTarget)}><Plus /> New</Button>
						<Button variant="outline" size="sm" onclick={(event) => void beginNameAction('duplicate', event.currentTarget)}><Copy /> Duplicate</Button>
						<Button variant="outline" size="sm" onclick={(event) => void beginNameAction('rename', event.currentTarget)}><Pencil /> Rename</Button>
						<Button variant="outline" size="sm" disabled={draft.customViews.length <= 1} onclick={(event) => beginDelete(event.currentTarget)}><Trash2 /> Delete</Button>
					</div>
					{#if deletePending}
						<div class="mt-3 flex flex-wrap items-center gap-2 rounded-lg border border-red-800 bg-red-950/40 p-3" role="alert">
							<p class="min-w-48 flex-1 text-sm">Delete <strong>{activeView.name}</strong>? This takes effect when you apply the dashboard.</p>
							<Button variant="destructive" size="sm" onclick={removeActiveView}>Delete view</Button>
							<Button variant="ghost" size="sm" onclick={cancelDelete}>Cancel</Button>
						</div>
					{/if}

					{#if nameAction}
						<div class="mt-3 flex flex-wrap items-end gap-2 rounded-lg border border-border bg-muted/30 p-3">
							<div class="min-w-48 flex-1">
								<label for="dashboard-view-name" class="mb-1 block text-xs font-medium">{nameAction === 'rename' ? 'View name' : nameAction === 'duplicate' ? 'Duplicate name' : 'New view name'}</label>
								<input bind:this={nameInput} id="dashboard-view-name" bind:value={nameDraft} onkeydown={handleNameKeydown} maxlength={DASHBOARD_VIEW_NAME_MAX_LENGTH} aria-invalid={nameDraft.trim() && !nameAvailable ? 'true' : undefined} aria-describedby={nameDraft.trim() && !nameAvailable ? 'dashboard-view-name-error' : undefined} placeholder="Dashboard view name" class="h-9 w-full rounded-md border border-border bg-background px-3 text-sm" />
							</div>
							<Button size="sm" disabled={!nameAvailable} onclick={submitNameAction}>{nameAction === 'rename' ? 'Rename' : nameAction === 'duplicate' ? 'Duplicate' : 'Create'}</Button>
							<Button variant="ghost" size="sm" onclick={() => cancelNameAction(true)}>Cancel</Button>
							{#if nameDraft.trim() && !nameAvailable}<p id="dashboard-view-name-error" class="w-full text-xs text-red-400" role="alert">Use a unique view name.</p>{/if}
						</div>
					{/if}
				</div>

				<div class="mt-6 flex flex-wrap items-center justify-between gap-3 border-t border-border pt-5">
					<div><h3 class="text-sm font-semibold">{activeView.name} widgets</h3><p class="text-xs text-muted-foreground">Widgets use a 12-column wide-screen grid and can span the full row, one half, one third, or one quarter.</p></div>
					<div class="flex items-center gap-2">
						<label for="dashboard-density" class="text-xs font-medium text-muted-foreground">Density</label>
						<select id="dashboard-density" value={activeView.density} onchange={(event) => updateDensity(event.currentTarget.value)} class="h-8 rounded-md border border-border bg-background px-2 text-xs">
							<option value="comfortable">Comfortable</option><option value="compact">Compact</option>
						</select>
					</div>
				</div>

				<div class="mt-3 divide-y divide-border rounded-lg border border-border">
					{#each activeView.widgets as widget, index (widget.id)}
						<div class="flex flex-wrap items-center gap-3 px-3 py-2.5">
							<input type="checkbox" checked={widget.visible} disabled={widget.visible && activeView.widgets.filter((candidate) => candidate.visible).length === 1} onchange={(event) => updateWidget(index, { visible: event.currentTarget.checked })} aria-label={`Show ${dashboardWidgetMeta[widget.id].label}`} class="h-4 w-4 accent-primary disabled:opacity-40" />
							<div class="min-w-40 flex-1"><div class="text-sm font-medium">{dashboardWidgetMeta[widget.id].label}</div><div class="truncate text-xs text-muted-foreground">{dashboardWidgetMeta[widget.id].description}</div></div>
							{#if dashboardWidgetSupportsTiny(widget.id)}
								<select value={widget.presentation} disabled={!widget.visible} onchange={(event) => updatePresentation(index, event.currentTarget.value)} aria-label={`${dashboardWidgetMeta[widget.id].label} presentation`} class="h-8 rounded-md border border-border bg-background px-2 text-xs disabled:opacity-40"><option value="standard">Standard</option><option value="tiny">Tiny</option></select>
							{/if}
							<select value={widget.width} disabled={!widget.visible} onchange={(event) => updateWidget(index, { width: parseWidgetWidth(event.currentTarget.value) })} aria-label={`${dashboardWidgetMeta[widget.id].label} width`} class="h-8 rounded-md border border-border bg-background px-2 text-xs disabled:opacity-40"><option value="full">Full</option><option value="half">1/2</option>{#if dashboardWidgetSupportsNarrowWidth(widget.id, widget.presentation)}<option value="third">1/3</option><option value="quarter">1/4</option>{/if}</select>
							<div class="flex">
								<Button variant="ghost" size="icon-sm" disabled={index === 0} onclick={() => moveWidget(index, -1)} aria-label={`Move ${dashboardWidgetMeta[widget.id].label} up`}><ArrowUp /></Button>
								<Button variant="ghost" size="icon-sm" disabled={index === activeView.widgets.length - 1} onclick={() => moveWidget(index, 1)} aria-label={`Move ${dashboardWidgetMeta[widget.id].label} down`}><ArrowDown /></Button>
							</div>
						</div>
					{/each}
				</div>
			{/if}
			{#if invalidView}<p class="mt-3 text-xs text-red-400">Select at least one widget in {invalidView.name}.</p>{/if}
		</div>

		<Dialog.Footer class="border-t border-border px-6 py-4">
			{#if draft.preset === 'custom'}<Button variant="ghost" onclick={resetCustom} class="mr-auto"><RotateCcw /> Reset view</Button>{/if}
			<Button variant="outline" onclick={() => open = false}>Cancel</Button>
			<Button disabled={!canSave} onclick={save}>Apply dashboard</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
