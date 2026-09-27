<script lang="ts">
	interface Props {
		name: string;
		fallbackName: string;
		prefix: string;
	}
	let { name, fallbackName, prefix }: Props = $props();
</script>

<span class="text-muted-foreground">Target</span>
<span class="font-mono text-xs">{prefix}:{(name || fallbackName).toLowerCase()}</span>
