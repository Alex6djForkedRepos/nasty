<script lang="ts">
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';

	interface Props {
		name: string;
		prefix: string;
	}
	let { name = $bindable(), prefix }: Props = $props();
</script>

<div class="mb-4">
	<Label>Target Name</Label>
	<Input bind:value={name} placeholder="dbserver" class="mt-1" />
	<p class="mt-1 text-xs text-muted-foreground">IQN: {prefix}:{name.toLowerCase() || '...'}</p>
</div>
