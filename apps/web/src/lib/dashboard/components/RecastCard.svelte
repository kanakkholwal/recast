<script lang="ts">
	import {
		formatBytes,
		formatCount,
		formatDuration,
		formatRelative,
	} from "$lib/dashboard/format";
	import type { Recast } from "$lib/dashboard/store.svelte";
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
	} from "@lucide/svelte";

	let {
		recast,
		onplay,
		onrename,
		oncopylink,
		ontogglesource,
		ondelete,
	}: {
		recast: Recast;
		onplay: () => void;
		onrename: () => void;
		oncopylink: () => void;
		ontogglesource: () => void;
		ondelete: () => void;
	} = $props();

	let posterFailed = $state(false);
	const showPoster = $derived(!!recast.posterUrl && !posterFailed);
</script>

<article
	class="glass-card group/card relative flex h-full flex-col overflow-hidden rounded-xl transition-shadow duration-300 hover:shadow-craft-lg"
>
	<!-- Thumbnail (fixed height — robust across grid breakpoints) -->
	<button
		type="button"
		onclick={onplay}
		aria-label="Play {recast.title}"
		class="relative block h-44 w-full shrink-0 overflow-hidden bg-foreground/5"
	>
		{#if showPoster}
			<img
				src={recast.posterUrl}
				alt=""
				loading="lazy"
				onerror={() => (posterFailed = true)}
				class="absolute inset-0 h-full w-full object-cover transition-transform duration-500 group-hover/card:scale-[1.04]"
			/>
		{:else}
			<!-- Empty-poster state. Inherits the techy framing from the home
			     editor-tour rail: dot grid + primary glow blob + corner
			     brackets + a glass-tile icon. Reads as "ready for a frame"
			     instead of "missing image". -->
			<div
				aria-hidden="true"
				class="absolute inset-0 opacity-60"
				style="background-image: radial-gradient(circle, color-mix(in srgb, var(--color-foreground) 8%, transparent) 1px, transparent 1px); background-size: 16px 16px;"
			></div>
			<div
				aria-hidden="true"
				class="pointer-events-none absolute -bottom-10 left-1/2 size-44 -translate-x-1/2 rounded-full opacity-70"
				style="background: radial-gradient(closest-side, color-mix(in srgb, var(--color-primary) 22%, transparent), transparent 75%);"
			></div>
			<div class="absolute inset-0 grid place-items-center">
				<span class="grid size-16 place-items-center rounded-xl border border-border-low/60 bg-background/55 shadow-craft-sm backdrop-blur-sm">
					<Film class="size-7 text-foreground/70 drop-shadow-[0_4px_12px_color-mix(in_srgb,var(--color-primary)_35%,transparent)]" />
				</span>
			</div>
		{/if}

		<!-- CRT-style corner brackets. Always-on accent that ties the card to
		     the marketing rail's visual language without obscuring posters. -->
		<span aria-hidden="true" class="pointer-events-none absolute left-2 top-2 z-10 size-2.5 border-l border-t border-foreground/35"></span>
		<span aria-hidden="true" class="pointer-events-none absolute right-2 top-2 z-10 size-2.5 border-r border-t border-foreground/35"></span>
		<span aria-hidden="true" class="pointer-events-none absolute bottom-2 left-2 z-10 size-2.5 border-b border-l border-foreground/35"></span>
		<span aria-hidden="true" class="pointer-events-none absolute bottom-2 right-2 z-10 size-2.5 border-b border-r border-foreground/35"></span>

		<!-- Play overlay -->
		<span class="absolute inset-0 grid place-items-center bg-background/35 opacity-0 backdrop-blur-[1px] transition-opacity duration-300 group-hover/card:opacity-100">
			<span class="grid size-12 place-items-center rounded-full bg-primary text-background shadow-craft-floating transition-transform duration-200 group-active/card:scale-95">
				<Play class="size-5 translate-x-0.5 fill-current" />
			</span>
		</span>

		<!-- Duration. Mono-tag style matches the editor-tour chips. -->
		<span class="absolute bottom-2.5 right-2.5 z-20 flex items-center gap-1 rounded-md bg-background/85 px-1.5 py-0.5 font-mono text-[10px] font-semibold tabular-nums text-foreground ring-1 ring-inset ring-border-low/50 backdrop-blur-sm">
			<Clock class="size-3" />
			{formatDuration(recast.durationSec)}
		</span>

		<!-- Source. Same mono-tag rhythm as the duration chip; primary tint
		     for cloud, neutral for local. -->
		<span
			class="absolute left-2.5 top-2.5 z-20 flex items-center gap-1 rounded-md px-1.5 py-0.5 font-mono text-[10px] font-bold uppercase tracking-wider ring-1 ring-inset backdrop-blur-sm
				{recast.source === 'cloud'
				? 'bg-primary/90 text-background ring-primary/40'
				: 'bg-background/85 text-muted-foreground ring-border-low/50'}"
		>
			{#if recast.source === "cloud"}
				<Cloud class="size-3" />{recast.provider}
			{:else}
				<MonitorPlay class="size-3" />Local
			{/if}
		</span>
	</button>

	<!-- Meta -->
	<div class="flex flex-1 items-start gap-2 p-4">
		<div class="min-w-0 flex-1">
			<h3 class="truncate text-sm font-semibold text-foreground" title={recast.title}>
				{recast.title}
			</h3>
			<p class="mt-1 text-xs text-muted-foreground">
				{formatRelative(recast.createdAt)} · {formatBytes(recast.sizeBytes)}{#if recast.source === "cloud"} · {formatCount(recast.views)} views{/if}
			</p>
		</div>

		<DropdownMenu.Root>
			<DropdownMenu.Trigger
				class="grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground outline-none transition-colors hover:bg-foreground/8 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50"
				aria-label="Recast options"
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
					{#if recast.source === "cloud"}
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
