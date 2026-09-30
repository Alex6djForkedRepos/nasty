<script lang="ts">
	import { onMount } from 'svelte';
	import { mediaPreviewKind } from '$lib/public-share';

	let { url, name, onclose }: { url: string; name: string; onclose: () => void } = $props();
	let details = $state('Inspecting media with Mediabunny…');
	let inspectionError = $state('');
	let playbackError = $state(false);
	let thumbnail = $state<HTMLCanvasElement>();
	let hasThumbnail = $state(false);
	const kind = $derived(mediaPreviewKind(name));

	onMount(() => {
		let cancelled = false;
		const controller = new AbortController();
		let input: import('mediabunny').Input | undefined;
		// Metadata/frame probing is capped independently of native playback.
		// Never load the whole file merely to inspect a large video.
		let readBytes = 0;
		const deadline = setTimeout(() => {
			controller.abort();
			input?.dispose();
		}, 15000);
		void (async () => {
			try {
				const m = await import('$lib/media-readers');
				if (cancelled) return;
				input = new m.Input({
					formats: [m.MP4, m.QTFF, m.MATROSKA, m.WEBM, m.MP3, m.WAVE, m.OGG, m.FLAC, m.ADTS],
					source: new m.CustomSource({
						maxCacheSize: 4 * 1024 * 1024,
						getSize: async () => {
							const response = await fetch(url, { method: 'HEAD', signal: controller.signal, redirect: 'error' });
							if (!response.ok) throw new Error('Preview access expired or this file is unavailable.');
							const length = Number(response.headers.get('Content-Length'));
							if (!Number.isSafeInteger(length) || length <= 0) throw new Error('Media size is unavailable.');
							return length;
						},
						read: async (start, end) => {
							if (end - start > 1024 * 1024 || readBytes + end - start > 16 * 1024 * 1024) {
								throw new Error('Media inspection exceeded the prototype read budget.');
							}
							readBytes += end - start;
							const response = await fetch(url, {
								headers: { Range: `bytes=${start}-${end - 1}` },
								signal: controller.signal, redirect: 'error'
							});
							if (response.status !== 206 || Number(response.headers.get('Content-Length')) !== end - start) {
								await response.body?.cancel();
								throw new Error('Media range access failed or the share expired.');
							}
							const bytes = new Uint8Array(await response.arrayBuffer());
							if (bytes.length !== end - start) throw new Error('Media changed while reading.');
							return bytes;
						}
					})
				});
				const duration = await input.getDurationFromMetadata();
				const video = await input.getPrimaryVideoTrack();
				const audio = await input.getPrimaryAudioTrack();
				const codecs = await Promise.all([video?.getCodec(), audio?.getCodec()]);
				if (cancelled) return;
				details = [duration != null ? `${duration.toFixed(1)} seconds` : 'Duration unavailable', ...codecs.filter(Boolean)].join(' · ');
				if (video && await video.canDecode()) {
					const sink = new m.CanvasSink(video, { width: 480 });
					const frame = await sink.getCanvas(Math.max(0, await video.getFirstTimestamp()));
					if (!cancelled && frame && thumbnail) {
						thumbnail.width = frame.canvas.width;
						thumbnail.height = frame.canvas.height;
						thumbnail.getContext('2d')?.drawImage(frame.canvas, 0, 0);
						hasThumbnail = true;
					}
				}
			} catch (error) {
				if (!cancelled) {
					if (details.startsWith('Inspecting')) details = 'Media metadata unavailable.';
					inspectionError = error instanceof Error ? error.message : 'Media inspection unavailable.';
				}
			} finally {
				clearTimeout(deadline);
				input?.dispose();
			}
		})();
		return () => {
			cancelled = true;
			clearTimeout(deadline);
			controller.abort();
			input?.dispose();
		};
	});
</script>

<section class="my-4 space-y-3 rounded-lg border border-border p-4" aria-label="Media preview">
	<div class="flex items-center justify-between gap-3">
		<h2 class="min-w-0 truncate font-medium">{name}</h2>
		<button type="button" onclick={onclose} class="rounded-md border px-3 py-1 text-sm">Close preview</button>
	</div>
	<p class="text-xs text-muted-foreground">Prototype · Native playback; Mediabunny metadata and frame preview. Browser codec support varies.</p>
	{#if kind === 'video'}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video src={url} controls preload="metadata" playsinline onerror={() => playbackError = true} class="max-h-96 w-full rounded bg-black"></video>
	{:else}
		<audio src={url} controls preload="metadata" onerror={() => playbackError = true} class="w-full"></audio>
	{/if}
	{#if playbackError}<p class="text-sm text-muted-foreground">This browser could not play the file, or preview access ended. You can still try downloading it.</p>{/if}
	<p class="text-xs text-muted-foreground">{details}</p>
	<canvas bind:this={thumbnail} class:hidden={!hasThumbnail} class="max-h-48 max-w-full rounded" aria-label="Mediabunny decoded first frame"></canvas>
	{#if inspectionError}<p class="text-xs text-muted-foreground">{inspectionError}</p>{/if}
</section>
