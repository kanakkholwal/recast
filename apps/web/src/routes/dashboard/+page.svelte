<script lang="ts">
	import { formatBytes } from "$lib/dashboard/format";
	import PlayerDialog from "$lib/dashboard/components/PlayerDialog.svelte";
	import RecordingCard from "$lib/dashboard/components/RecordingCard.svelte";
	import RenameDialog from "$lib/dashboard/components/RenameDialog.svelte";
	import StatCard from "$lib/dashboard/components/StatCard.svelte";
	import {
		recordingsStore,
		settingsStore,
		type Recording,
		type RecordingSource,
	} from "$lib/dashboard/store.svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import { Cloud, Film, HardDrive, Search, Upload, Video, X } from "lucide-svelte";
	import { flip } from "svelte/animate";
	import { cubicOut } from "svelte/easing";
	import { fly, scale } from "svelte/transition";

	type SortKey = "recent" | "oldest" | "name" | "largest";

	let query = $state("");
	let activeFilter = $state<RecordingSource | "all">("all");
	let sortKey = $state<SortKey>("recent");

	let playing = $state<Recording | null>(null);
	let renaming = $state<Recording | null>(null);
	let uploading = $state(false);
	let fileInput = $state<HTMLInputElement | null>(null);

	const filters: { label: string; value: RecordingSource | "all" }[] = [
		{ label: "All", value: "all" },
		{ label: "Cloud", value: "cloud" },
		{ label: "Local", value: "local" },
	];

	const sorts: { label: string; value: SortKey }[] = [
		{ label: "Newest first", value: "recent" },
		{ label: "Oldest first", value: "oldest" },
		{ label: "Name (A–Z)", value: "name" },
		{ label: "Largest first", value: "largest" },
	];

	const visible = $derived.by(() => {
		const q = query.trim().toLowerCase();
		const list = recordingsStore.items.filter(
			(r) =>
				(activeFilter === "all" || r.source === activeFilter) &&
				r.title.toLowerCase().includes(q),
		);
		return [...list].sort((a, b) => {
			switch (sortKey) {
				case "oldest":
					return a.createdAt - b.createdAt;
				case "name":
					return a.title.localeCompare(b.title);
				case "largest":
					return b.sizeBytes - a.sizeBytes;
				default:
					return b.createdAt - a.createdAt;
			}
		});
	});

	const stats = $derived([
		{ icon: Video, label: "Recordings", value: String(recordingsStore.items.length) },
		{ icon: HardDrive, label: "Storage used", value: formatBytes(recordingsStore.usedBytes) },
		{ icon: Cloud, label: "On cloud", value: String(recordingsStore.cloudCount) },
	]);

	const hasRecordings = $derived(recordingsStore.items.length > 0);

	function readDuration(url: string): Promise<number> {
		return new Promise((resolve) => {
			const v = document.createElement("video");
			v.preload = "metadata";
			v.onloadedmetadata = () => resolve(v.duration || 0);
			v.onerror = () => resolve(0);
			v.src = url;
		});
	}

	async function onFilePicked(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		uploading = true;
		const url = URL.createObjectURL(file);
		const durationSec = await readDuration(url);
		const destination = settingsStore.value.preferences.defaultDestination;

		recordingsStore.add({
			id: crypto.randomUUID(),
			title: file.name.replace(/\.[^.]+$/, "") || "Untitled recording",
			durationSec,
			createdAt: Date.now(),
			sizeBytes: file.size,
			source: destination,
			provider: destination === "cloud" ? "Cloudinary" : null,
			views: 0,
			videoUrl: url,
			posterUrl: "",
		});

		uploading = false;
		input.value = "";
		toast.success(`“${file.name}” added to your library.`);
	}

	function doRename(rec: Recording, title: string) {
		recordingsStore.rename(rec.id, title);
		renaming = null;
		toast.success("Recording renamed.");
	}

	function toggleSource(rec: Recording) {
		const next: RecordingSource = rec.source === "cloud" ? "local" : "cloud";
		recordingsStore.setSource(rec.id, next);
		toast.success(
			next === "cloud" ? "Uploaded to Cloudinary." : "Moved to local storage.",
		);
	}

	async function copyLink(rec: Recording) {
		try {
			await navigator.clipboard.writeText(
				`https://recast.nexonauts.com/v/${rec.id}`,
			);
			toast.success("Share link copied to clipboard.");
		} catch {
			toast.error("Couldn't access the clipboard.");
		}
	}

	function deleteRecording(rec: Recording) {
		recordingsStore.remove(rec.id);
		if (playing?.id === rec.id) playing = null;
		toast.success(`“${rec.title}” deleted.`);
	}
</script>

<svelte:head>
	<title>Recordings — Recast Dashboard</title>
</svelte:head>

<input
	bind:this={fileInput}
	type="file"
	accept="video/*"
	class="hidden"
	onchange={onFilePicked}
/>

<!-- Header -->
<header
	class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between"
	in:fly={{ y: 12, duration: 500, easing: cubicOut }}
>
	<div>
		<h1 class="text-2xl font-semibold tracking-tight text-foreground">
			Recordings
		</h1>
		<p class="mt-1 text-sm text-muted-foreground">
			Everything you've captured and uploaded — in one place.
		</p>
	</div>
	<Button class="gap-2" disabled={uploading} onclick={() => fileInput?.click()}>
		<Upload class="size-4" />
		{uploading ? "Adding…" : "Upload recording"}
	</Button>
</header>

<!-- Stats -->
<div class="mt-7 grid grid-cols-1 gap-3 sm:grid-cols-3">
	{#each stats as stat, i (stat.label)}
		<div in:fly={{ y: 12, duration: 480, delay: 80 + i * 70, easing: cubicOut }}>
			<StatCard icon={stat.icon} label={stat.label} value={stat.value} />
		</div>
	{/each}
</div>

<!-- Toolbar -->
<div
	class="mt-8 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between"
	in:fly={{ y: 12, duration: 480, delay: 280, easing: cubicOut }}
>
	<div class="flex items-center gap-2 rounded-lg border border-border-low/70 bg-card/50 px-3 py-2 backdrop-blur-sm lg:w-72">
		<Search class="size-4 shrink-0 text-muted-foreground" />
		<input
			type="text"
			bind:value={query}
			placeholder="Search recordings…"
			class="w-full bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground/70"
		/>
		{#if query}
			<button
				type="button"
				onclick={() => (query = "")}
				aria-label="Clear search"
				class="grid size-5 place-items-center rounded text-muted-foreground transition-colors hover:text-foreground"
			>
				<X class="size-3.5" />
			</button>
		{/if}
	</div>

	<div class="flex items-center gap-2">
		<div class="flex items-center gap-1 rounded-lg border border-border-low/60 bg-card/40 p-1">
			{#each filters as f (f.value)}
				<button
					type="button"
					onclick={() => (activeFilter = f.value)}
					class="rounded-md px-3 py-1.5 text-xs font-semibold transition-colors duration-200
						{activeFilter === f.value
						? 'bg-primary/12 text-foreground'
						: 'text-muted-foreground hover:text-foreground'}"
				>
					{f.label}
				</button>
			{/each}
		</div>

		<select
			bind:value={sortKey}
			aria-label="Sort recordings"
			class="rounded-lg border border-border-low/60 bg-card/40 px-3 py-2 text-xs font-semibold text-foreground outline-none transition-colors hover:border-border-low focus:border-primary/60"
		>
			{#each sorts as s (s.value)}
				<option value={s.value}>{s.label}</option>
			{/each}
		</select>
	</div>
</div>

<!-- Grid -->
{#if visible.length > 0}
	<div class="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
		{#each visible as rec (rec.id)}
			<div
				animate:flip={{ duration: 320, easing: cubicOut }}
				in:scale={{ start: 0.97, duration: 300, easing: cubicOut }}
				out:scale={{ start: 0.97, duration: 170, easing: cubicOut }}
			>
				<RecordingCard
					recording={rec}
					onplay={() => (playing = rec)}
					onrename={() => (renaming = rec)}
					oncopylink={() => copyLink(rec)}
					ontogglesource={() => toggleSource(rec)}
					ondelete={() => deleteRecording(rec)}
				/>
			</div>
		{/each}
	</div>
{:else}
	<div
		class="mt-6 flex flex-col items-center justify-center rounded-xl border border-dashed border-border-low/70 py-20 text-center"
		in:fly={{ y: 12, duration: 360, easing: cubicOut }}
	>
		<span class="glass-chip grid size-12 place-items-center rounded-xl text-muted-foreground">
			<Film class="size-5" />
		</span>
		{#if hasRecordings}
			<h3 class="mt-4 text-sm font-semibold text-foreground">No recordings found</h3>
			<p class="mt-1 max-w-xs text-xs text-muted-foreground">
				Nothing matches your search and filters.
			</p>
			<Button
				variant="outline"
				size="sm"
				class="mt-5"
				onclick={() => {
					query = "";
					activeFilter = "all";
				}}
			>
				Clear filters
			</Button>
		{:else}
			<h3 class="mt-4 text-sm font-semibold text-foreground">No recordings yet</h3>
			<p class="mt-1 max-w-xs text-xs text-muted-foreground">
				Upload a video, or capture one with the Recast desktop app.
			</p>
			<Button size="sm" class="mt-5 gap-2" onclick={() => fileInput?.click()}>
				<Upload class="size-3.5" />
				Upload recording
			</Button>
		{/if}
	</div>
{/if}

{#if playing}
	<PlayerDialog recording={playing} onclose={() => (playing = null)} />
{/if}

{#if renaming}
	<RenameDialog
		recording={renaming}
		onclose={() => (renaming = null)}
		onsave={(title) => renaming && doRename(renaming, title)}
	/>
{/if}
