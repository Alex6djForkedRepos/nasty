<script lang="ts">
	interface Props {
		name: string;
		fallbackName: string;
		addr: string;
		port: string;
		prefix: string;
	}
	let { name, fallbackName, addr, port, prefix }: Props = $props();
</script>

<span class="text-muted-foreground">Subsystem</span>
<span class="font-mono text-xs">{prefix}:{name || fallbackName}</span>
<span class="text-muted-foreground">Listen</span>
<span>{addr}:{port}</span>
