<script lang="ts">
	import {
		formatBytes,
		formatCount,
		formatDuration,
		formatRelative,
	} from "$lib/dashboard/format";
	import type { Recording } from "$lib/dashboard/store.svelte";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import {
		Clock,
		Cloud,
		CloudUpload,
		Film,
		HardDrive,
		Link2,
		MonitorPlay,
		MoreHorizontal,
		Pencil,
		Play,
		Trash2,
	} from "lucide-svelte";

	let {
		recording,
		onplay,
		onrename,
		oncopylink,
		ontogglesource,
		ondelete,
	}: {
		recording: Recording;
		onplay: () => void;
		onrename: () => void;
		oncopylink: () => void;
		ontogglesource: () => void;
		ondelete: () => void;
	} = $props();

	let posterFailed = $state(false);
	const showPoster = $derived(!!recording.posterUrl && !posterFailed);
</script>

<article
	class="glass-card group/card relative flex h-full flex-col overflow-hidden rounded-xl transition-shadow duration-300 hover:shadow-craft-lg"
>
	<!-- Thumbnail (fixed height — robust across grid breakpoints) -->
	<button
		type="button"
		onclick={onplay}
		aria-label="Play {recording.title}"
		class="relative block h-44 w-full shrink-0 overflow-hidden bg-foreground/5"
	>
		{#if showPoster}
			<img
				src={recording.posterUrl}
				alt=""
				loading="lazy"
				onerror={() => (posterFailed = true)}
				class="absolute inset-0 h-full w-full object-cover transition-transform duration-500 group-hover/card:scale-[1.04]"
			/>
		{:else}
			<div class="absolute inset-0 bg-linear-to-br from-primary/25 via-tertiary/15 to-transparent"></div>
			<div class="bg-grid bg-grid-fade absolute inset-0 opacity-40"></div>
			<div class="absolute inset-0 grid place-items-center">
				<Film class="size-8 text-foreground/25" />
			</div>
		{/if}

		<!-- Play overlay -->
		<span class="absolute inset-0 grid place-items-center bg-background/35 opacity-0 backdrop-blur-[1px] transition-opacity duration-300 group-hover/card:opacity-100">
			<span class="grid size-12 place-items-center rounded-full bg-primary text-background shadow-craft-floating transition-transform duration-200 group-active/card:scale-95">
				<Play class="size-5 translate-x-0.5 fill-current" />
			</span>
		</span>

		<!-- Duration -->
		<span class="absolute bottom-2 right-2 flex items-center gap-1 rounded-md bg-background/85 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-foreground backdrop-blur-sm">
			<Clock class="size-3" />
			{formatDuration(recording.durationSec)}
		</span>

		<!-- Source -->
		<span
			class="absolute left-2 top-2 flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider backdrop-blur-sm
				{recording.source === 'cloud'
				? 'bg-primary/90 text-background'
				: 'bg-background/85 text-muted-foreground'}"
		>
			{#if recording.source === "cloud"}
				<Cloud class="size-3" />{recording.provider}
			{:else}
				<MonitorPlay class="size-3" />Local
			{/if}
		</span>
	</button>

	<!-- Meta -->
	<div class="flex flex-1 items-start gap-2 p-4">
		<div class="min-w-0 flex-1">
			<h3 class="truncate text-sm font-semibold text-foreground" title={recording.title}>
				{recording.title}
			</h3>
			<p class="mt-1 text-xs text-muted-foreground">
				{formatRelative(recording.createdAt)} · {formatBytes(recording.sizeBytes)}{#if recording.source === "cloud"} · {formatCount(recording.views)} views{/if}
			</p>
		</div>

		<DropdownMenu.Root>
			<DropdownMenu.Trigger
				class="grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground outline-none transition-colors hover:bg-foreground/8 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
				aria-label="Recording options"
			>
				<MoreHorizontal class="size-4" />
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end" sideOffset={6} class="w-48">
				<DropdownMenu.Item onclick={onplay}>
					<Play class="size-4 text-muted-foreground" />
					Play
				</DropdownMenu.Item>
				<DropdownMenu.Item onclick={onrename}>
					<Pencil class="size-4 text-muted-foreground" />
					Rename
				</DropdownMenu.Item>
				<DropdownMenu.Item onclick={oncopylink}>
					<Link2 class="size-4 text-muted-foreground" />
					Copy link
				</DropdownMenu.Item>
				<DropdownMenu.Item onclick={ontogglesource}>
					{#if recording.source === "cloud"}
						<HardDrive class="size-4 text-muted-foreground" />
						Move to local
					{:else}
						<CloudUpload class="size-4 text-muted-foreground" />
						Upload to cloud
					{/if}
				</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item
					onclick={ondelete}
					class="text-destructive/90 data-highlighted:text-destructive"
				>
					<Trash2 class="size-4" />
					Delete
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	</div>
</article>
