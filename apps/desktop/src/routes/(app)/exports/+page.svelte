<script lang="ts">
import { goto } from "$app/navigation";
import ShareManageDialog from "$components/cloud/ShareManageDialog.svelte";
import WorkspacePickerDialog from "$components/cloud/WorkspacePickerDialog.svelte";
import { ConfirmDialog, PlayerDialog, RenameDialog } from "$components/recast";
import { formatSize, getExtension, isImageFile } from "$lib/format/files";
import {
	deleteFile,
	listExports,
	openFileLocation,
	renameFile,
	type RecordingEntry,
} from "$lib/ipc";
import { filterEntries, sortEntries, sumBytes, type LibrarySort } from "$lib/library/list";
import { createSelection } from "$lib/library/selection.svelte";
import {
	createThumbnailLoader,
	libraryDate,
	removeThumbnail,
	removeThumbnails,
	renameThumbnail,
} from "$lib/library/thumbnails";
import { morph } from "$lib/morph";
import { isShareSupported, shareRecording } from "$lib/share";
import { shareTargetFor } from "$lib/share-target";
import { platform } from "@tauri-apps/plugin-os";
import RecastMark from "$components/recast-mark.svelte";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import { gdrive } from "$lib/stores/gdrive.svelte";
import {
	Check,
	Clock,
	CopyIcon,
	Download,
	ExternalLink,
	Eye,
	FolderOpen,
	Grid3x3,
	BrandGoogleDrive,
	Link2,
	List,
	ListChecks,
	MoreHorizontal,
	Pencil,
	Play,
	RefreshCw,
	Search,
	SlidersHorizontal,
	SortAsc,
	Trash2,
	Unlink2,
	X,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { ButtonGroup } from "@recast/ui/button-group";
import { Cutout } from "@recast/ui/cutout";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { safeStorage } from "@recast/ui/persisted-state";
import * as Select from "@recast/ui/select";
import { Skeleton } from "@recast/ui/skeleton";
import { toast } from "@recast/ui/sonner";
import { cn } from "@recast/ui/utils";
import { onMount } from "svelte";
import { cubicOut } from "svelte/easing";
import { fade, fly } from "svelte/transition";

let entries = $state<RecordingEntry[]>([]);
let isLoading = $state(true);
let thumbnails = $state<Record<string, string>>({});
const loadThumbnails = createThumbnailLoader();

let query = $state("");
let view = $state<"grid" | "list">("grid");
let sort = $state<LibrarySort>("recent");
let renameTarget = $state<RecordingEntry | null>(null);
let deleteTarget = $state<RecordingEntry | null>(null);
let manageTarget = $state<RecordingEntry | null>(null);
let playTarget = $state<RecordingEntry | null>(null);
// Set when a share needs a workspace choice (user is in >1 workspace).
let workspacePick = $state<{ path: string; title: string; fileName: string } | null>(null);

// Multi-select: a toolbar "Select" toggle flips the page into selection
// mode, where clicking a card checks it instead of opening the file.
let bulkDeleteOpen = $state(false);
const selection = createSelection({
	noun: "export",
	deleteFile,
	onDeleted: (deleted) => {
		entries = entries.filter((e) => !deleted.has(e.path));
		if (deleted.size > 0) thumbnails = removeThumbnails(thumbnails, deleted);
	},
});

onMount(() => {
	fetchExports();
	// Hydrate upload history so each row's dropdown picks the right action
	// (upload vs. copy-link/manage) without a per-row roundtrip.
	void gdrive.init();
	void cloudShare.init();
	view = safeStorage.get<"grid" | "list">("exports-view", view);
});

$effect(() => {
	safeStorage.set("exports-view", view);
});

async function fetchExports() {
	isLoading = true;
	try {
		entries = await listExports();
		void refreshThumbnails(entries);
	} catch (e) {
		toast.error(`Could not load exports: ${e}`);
	} finally {
		isLoading = false;
	}
}

async function refreshThumbnails(items: RecordingEntry[]) {
	const next = await loadThumbnails(items);
	if (next) thumbnails = next;
}

async function copyPath(entry: RecordingEntry) {
	try {
		await navigator.clipboard.writeText(entry.path);
		toast.success("Path copied");
	} catch (e) {
		toast.error(`Copy failed: ${e}`);
	}
}

async function handleRename(entry: RecordingEntry, nextName: string) {
	const newPath = await renameFile(entry.path, nextName);
	entries = entries.map((e) =>
		e.path === entry.path
			? {
					...e,
					path: newPath,
					filename: newPath.split(/[\\/]/).pop() ?? nextName,
				}
			: e,
	);
	thumbnails = renameThumbnail(thumbnails, entry.path, newPath);
	toast.success("Renamed");
}

async function handleDelete(entry: RecordingEntry) {
	await deleteFile(entry.path);
	entries = entries.filter((e) => e.path !== entry.path);
	thumbnails = removeThumbnail(thumbnails, entry.path);
	// Local file is gone, so drop its upload records so the row doesn't return
	// next session claiming a copy. The remote objects are left untouched.
	void gdrive.forgetUpload(entry.path);
	void cloudShare.forget(entry.path);
	toast.success(`Moved "${entry.filename}" to trash`);
}

/**
 * Share an export to Recast Cloud: upload, create a public link, copy it.
 * Routes to Settings when signed out, because device sign-in opens a browser tab
 * and shouldn't happen inline from a menu.
 */
async function shareToCloud(entry: RecordingEntry) {
	// Only block on the network before the store has hydrated; a loading toast
	// covers that first wait. Afterwards the cached workspace list is instant.
	if (!cloudShare.initialized) {
		const tid = toast.loading("Connecting to Recast Cloud…");
		await cloudShare.init();
		toast.dismiss(tid);
	}
	if (!cloudShare.signedIn) {
		toast.info("Sign in to Recast Cloud in Settings first.");
		void goto("/settings");
		return;
	}
	const title = entry.filename.replace(/\.[^.]+$/, "");
	// Multiple workspaces → confirm the target before the upload commits.
	if (cloudShare.workspaces.length > 1) {
		workspacePick = { path: entry.path, title, fileName: entry.filename };
		// Freshen plan/recast counts in the background while they choose.
		void cloudShare.refreshStatus();
		return;
	}
	beginCloudShare(entry.path, title);
}

/**
 * Start an upload + share and surface it in the global foreground dialog
 * (CloudShareHost). No toast: the store records phase/byte/result/error and the
 * dialog reads it live, so the store's rejection is swallowed here.
 *
 * The dialog is opened on the next frame so a closing overlay (the row's
 * dropdown, or the workspace picker) fully settles first. Opening a second
 * modal in the same tick makes bits-ui hand focus back and the new dialog never
 * appears, which read as "no dialog showed up".
 */
function beginCloudShare(path: string, title: string, workspaceId?: string) {
	void cloudShare.share(path, title, workspaceId).catch(() => {});
	requestAnimationFrame(() => cloudShare.setForeground(path));
}

async function copyCloudLink(entry: RecordingEntry) {
	const record = cloudShare.getRecordForPath(entry.path);
	if (!record) return;
	try {
		await navigator.clipboard.writeText(record.shareUrl);
		toast.success("Share link copied.");
	} catch (e) {
		toast.error(`Could not copy link: ${e}`);
	}
}

async function openCloudLink(entry: RecordingEntry) {
	const record = cloudShare.getRecordForPath(entry.path);
	if (!record) return;
	try {
		const { openUrl } = await import("@tauri-apps/plugin-opener");
		await openUrl(record.shareUrl);
	} catch {
		window.open(record.shareUrl, "_blank", "noopener");
	}
}

async function forgetCloudShare(entry: RecordingEntry) {
	await cloudShare.forget(entry.path);
	toast.success(`Forgot cloud link for "${entry.filename}"`);
}

/**
 * Drive upload from the exports list. Routes to Settings when Drive isn't
 * connected, because the consent flow opens a browser tab, not inline.
 */
async function uploadToDrive(entry: RecordingEntry) {
	await gdrive.init();
	if (!gdrive.connected) {
		toast.info("Connect Google Drive in Settings first.");
		void goto("/settings");
		return;
	}
	// Progress lives in the foreground dialog (and the activity center once
	// minimized), never in-place on the card. The store toasts the outcome.
	const id = gdrive.startUpload(entry.path);
	requestAnimationFrame(() => gdrive.setForeground(id));
}

// `navigator.share` exposure is static, so sample once at module load so the
// dropdown can conditionally render the Share item without a reactive read.
const shareSupported = isShareSupported();
// Capitalised binding so it reads as a component in markup.
const ShareIcon = shareTargetFor(platform()).icon;

/**
 * Open the OS share sheet for an export. Tries the file payload (Web Share
 * Level 2), falling back to the recorded Drive link if files can't be shared.
 */
async function shareEntry(entry: RecordingEntry) {
	const fallbackLink = gdrive.getRecordForPath(entry.path)?.webViewLink;
	const result = await shareRecording({
		path: entry.path,
		fileName: entry.filename,
		title: entry.filename,
		text: "Made with Recast",
		fallbackLink,
	});
	if (result.ok || result.reason === "cancelled") return;
	if (result.reason === "unsupported") {
		toast.error(
			fallbackLink
				? "Sharing isn't available on this device."
				: "Sharing files isn't available here. Upload to Drive first to share a link.",
		);
	} else {
		toast.error(`Share failed: ${result.message ?? "unknown error"}`);
	}
}

/** Copy the recorded Drive link from the local manifest (no network). */
async function copyDriveLink(entry: RecordingEntry) {
	const record = gdrive.getRecordForPath(entry.path);
	if (!record?.webViewLink) {
		toast.error("No Drive link recorded for this file.");
		return;
	}
	try {
		await navigator.clipboard.writeText(record.webViewLink);
		toast.success("Drive link copied.");
	} catch (e) {
		toast.error(`Could not copy link: ${e}`);
	}
}

// openUrl via the opener plugin; window.open fallback for the web build.
async function openDriveLink(entry: RecordingEntry) {
	const record = gdrive.getRecordForPath(entry.path);
	if (!record?.webViewLink) {
		toast.error("No Drive link recorded for this file.");
		return;
	}
	try {
		const { openUrl } = await import("@tauri-apps/plugin-opener");
		await openUrl(record.webViewLink);
	} catch {
		window.open(record.webViewLink, "_blank", "noopener");
	}
}

// Recovery path when the Drive file was deleted or no longer matches: drops
// the local association so the dropdown flips back to "Upload to Drive". The
// Drive object is left untouched.
async function forgetDriveLink(entry: RecordingEntry) {
	await gdrive.forgetUpload(entry.path);
	toast.success(`Forgot Drive link for "${entry.filename}"`);
}

const filtered = $derived(
	sortEntries(filterEntries(entries, query, { matchExtension: true }), sort),
);

const totalSize = $derived(sumBytes(entries));

const selectedCount = $derived(selection.count);
const allFilteredSelected = $derived(selection.allSelected(filtered));

// Grid and list share one keyed {#each}. Touching `view` here gives the
// each block a reason to re-run on a layout toggle (returning a fresh
// array each time), which is what makes `animate:morph` fire.
const displayed = $derived.by(() => {
	void view;
	return filtered.slice();
});

function activateEntry(entry: RecordingEntry) {
	if (selection.selectMode) selection.toggle(entry.path);
	else playTarget = entry;
}

function handleCardKeydown(e: KeyboardEvent, entry: RecordingEntry) {
	if (e.key === "Enter" || e.key === " ") {
		e.preventDefault();
		activateEntry(entry);
	}
}
</script>

<div class="h-full overflow-y-auto scrollbar-transparent no-scrollbar">
  <div class="mx-auto flex max-w-5xl flex-col gap-8 px-6 py-10">
    <!-- Hero -->
    <header
      in:fly={{ y: 12, duration: 320, easing: cubicOut }}
      class="flex flex-col gap-3"
    >
      <span
        class="inline-flex w-fit items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.15em] text-muted-foreground/80 backdrop-blur"
      >
        <Download class="size-3 text-primary" />
        Exports
      </span>
      <h1
        class="text-balance text-[28px] font-semibold leading-tight tracking-tight text-foreground md:text-[32px]"
      >
        <span
          class="bg-linear-to-r from-foreground to-foreground/55 bg-clip-text text-transparent"
        >
          {entries.length === 0
            ? "Nothing exported yet"
            : entries.length === 1
              ? "1 export"
              : `${entries.length} exports`}
        </span>
      </h1>
      <p class="text-[12.5px] leading-relaxed text-muted-foreground">
        {formatSize(totalSize)} on disk · open a file in its folder or send straight
        to a teammate.
      </p>
    </header>

    <!-- Search bar -->
    <label
      in:fly={{ y: 12, duration: 320, delay: 60, easing: cubicOut }}
      class="group/search flex h-12 items-center gap-3 rounded-xl border border-border/60 bg-card/70 px-4 shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:border-border hover:bg-card hover:shadow-craft-sm focus-within:border-border focus-within:bg-card focus-within:shadow-craft-sm"
    >
      <Search
        class="size-4 shrink-0 text-muted-foreground/70 transition-colors group-hover/search:text-foreground group-focus-within/search:text-foreground"
      />
      <input
        bind:value={query}
        type="text"
        placeholder="Search exports…"
        aria-label="Search exports"
        class="flex-1 bg-transparent text-[13px] font-medium text-foreground placeholder:text-muted-foreground/80 focus:outline-none"
      />
      {#if query}
        <Button
          variant="ghost"
          size="icon-sm"
          class="size-6"
          onclick={() => (query = "")}
          title="Clear search"
        >
          <X class="size-3" />
        </Button>
      {/if}
    </label>

    <!-- Section header -->
    <div
      in:fly={{ y: 12, duration: 320, delay: 120, easing: cubicOut }}
      class="flex flex-col gap-3"
    >
      <div class="flex items-center justify-between gap-3 px-1">
        <h2
          class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >
          {query ? `Results for “${query}”` : "All exports"}
        </h2>
        <div class="flex items-center gap-1.5">
          <Button
            variant={selection.selectMode ? "secondary" : "ghost"}
            size="xs"
            class={cn(
              "h-7 gap-1 text-[11px]",
              !selection.selectMode &&
                "text-muted-foreground hover:text-foreground",
            )}
            onclick={selection.toggleMode}
            disabled={entries.length === 0}
            title="Select multiple exports"
          >
            <ListChecks size={11} />
            {selection.selectMode ? "Done" : "Select"}
          </Button>

          <Select.Root
            type="single"
            value={sort}
            onValueChange={(v: string) => {
              if (v === "recent" || v === "name" || v === "size") sort = v;
            }}
          >
            <Select.Trigger
              size="sm"
              class="h-7 gap-1 rounded-lg border-border/50 px-2.5 text-[11px] font-medium text-muted-foreground hover:text-foreground"
              aria-label="Sort exports"
            >
              <span data-slot="select-value" class="flex items-center gap-1">
                <SortAsc size={11} />
                {sort === "recent"
                  ? "Recent"
                  : sort === "name"
                    ? "Name"
                    : "Size"}
              </span>
            </Select.Trigger>
            <Select.Content align="end" sideOffset={6} class="w-36 p-1">
              <Select.Item value="recent" label="Recent" class="text-[11.5px]">
                <Clock class="size-3 text-muted-foreground" />
                Recent
              </Select.Item>
              <Select.Item value="name" label="Name" class="text-[11.5px]">
                <SortAsc class="size-3 text-muted-foreground" />
                Name
              </Select.Item>
              <Select.Item value="size" label="Size" class="text-[11.5px]">
                <Download class="size-3 text-muted-foreground" />
                Size
              </Select.Item>
            </Select.Content>
          </Select.Root>

          <ButtonGroup>
            <Button
              variant={view === "grid" ? "secondary" : "ghost"}
              size="icon-sm"
              onclick={() => (view = "grid")}
              title="Grid view"
            >
              <Grid3x3 size={12} />
            </Button>
            <Button
              variant={view === "list" ? "secondary" : "ghost"}
              size="icon-sm"
              onclick={() => (view = "list")}
              title="List view"
            >
              <List size={12} />
            </Button>
          </ButtonGroup>

          <Button
            variant="ghost"
            size="icon-sm"
            onclick={fetchExports}
            disabled={isLoading}
            title="Refresh"
          >
            <RefreshCw size={12} class={isLoading ? "animate-spin" : ""} />
          </Button>
        </div>
      </div>

      {#if isLoading && entries.length === 0}
        <div
          class={cn(
            "grid gap-3",
            view === "grid"
              ? "grid-cols-2 sm:grid-cols-3 lg:grid-cols-4"
              : "grid-cols-1",
          )}
        >
          {#each Array.from({ length: 8 }) as _, i (i)}
            <Skeleton
              class={cn(view === "grid" ? "aspect-video" : "h-16")}
              style="animation-delay: {i * 80}ms"
            />
          {/each}
        </div>
      {:else if filtered.length === 0}
        <div
          in:fade={{ duration: 200 }}
          class="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border/60 bg-card/40 p-12 text-center"
        >
          <div
            class="flex size-12 items-center justify-center rounded-xl bg-foreground/5 text-muted-foreground"
          >
            <Download class="size-5" />
          </div>
          <div>
            <p class="text-[14px] font-semibold text-foreground">
              {query ? "No matches" : "Nothing exported yet"}
            </p>
            <p class="mt-1 text-[11.5px] text-muted-foreground">
              {query
                ? `Nothing matches "${query}".`
                : "Render a recording from the editor and it'll show up here."}
            </p>
          </div>
        </div>
      {:else}
        <!-- Grid and list share one keyed {#each} so each card is the same
             DOM node in both layouts and can morph between them. -->
        <div
          class={view === "grid"
            ? "grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4"
            : "flex flex-col gap-1.5"}
        >
          {#each displayed as entry, i (entry.path)}
            {@const isSelected = selection.has(entry.path)}
            {@const isImage = isImageFile(entry.filename)}
            <div
              in:fade={{ duration: 200, delay: Math.min(i * 25, 200) }}
              animate:morph={{ duration: 340 }}
              role="button"
              tabindex="0"
              aria-label={entry.filename}
              title={entry.filename}
              onclick={() => activateEntry(entry)}
              onkeydown={(e) => handleCardKeydown(e, entry)}
              class={cn(
                "group/card relative flex cursor-pointer overflow-hidden border shadow-(--shadow-craft-inset) outline-none transition-[background-color,border-color,box-shadow] duration-200 focus-visible:ring-2 focus-visible:ring-ring/60",
                view === "grid"
                  ? "flex-col rounded-xl"
                  : "flex-row items-center gap-3 rounded-lg p-1.5",
                isSelected
                  ? "border-primary/60 bg-primary/5"
                  : "border-border/40 bg-card hover:border-border hover:shadow-craft-sm",
              )}
            >
              <!-- Thumbnail -->
              <div
                class={cn(
                  "relative shrink-0 overflow-hidden bg-muted/40",
                  view === "grid"
                    ? "aspect-video w-full"
                    : "aspect-video w-22 rounded-md",
                )}
              >
                {#if thumbnails[entry.path]}
                  <img
                    src={thumbnails[entry.path]}
                    alt=""
                    draggable="false"
                    class="size-full object-cover transition-transform duration-300 group-hover/card:scale-[1.03]"
                  />
                {:else}
                  <div
                    class="grid size-full place-items-center text-muted-foreground/50"
                  >
                    {#if isImage}
                      <Eye class={view === "grid" ? "size-6" : "size-4"} />
                    {:else}
                      <Play
                        class={cn(
                          "translate-x-px",
                          view === "grid" ? "size-6" : "size-4",
                        )}
                      />
                    {/if}
                  </div>
                {/if}

                {#if selection.selectMode}
                  <div class="absolute left-1.5 top-1.5 z-10">
                    <span
                      class={cn(
                        "flex size-5 items-center justify-center rounded-md border backdrop-blur-md transition-all",
                        isSelected
                          ? "border-primary bg-primary text-primary-foreground"
                          : "border-border/70 bg-background/80",
                      )}
                    >
                      {#if isSelected}<Check size={12} />{/if}
                    </span>
                  </div>
                {:else if view === "grid"}
                  <div
                    class="pointer-events-none absolute inset-0 grid place-items-center bg-linear-to-t from-black/40 via-transparent to-transparent opacity-0 transition-opacity duration-200 group-hover/card:opacity-100"
                  >
                    <span
                      class="flex size-9 items-center justify-center rounded-full bg-background/85 text-foreground shadow-craft-sm backdrop-blur"
                    >
                      {#if isImage}
                        <Eye class="size-4" />
                      {:else}
                        <Play class="size-4 translate-x-px" />
                      {/if}
                    </span>
                  </div>
                {/if}

                {#if view === "grid"}
                  <Cutout
                    corner="bl"
                    surface="card"
                    radius={8}
                    class="flex items-center px-2.5 pt-2.5 pb-1"
                  >
                    <span
                      class="text-[8.5px] font-bold uppercase leading-none tracking-wider text-muted-foreground"
                    >
                      {getExtension(entry.filename)}
                    </span>
                  </Cutout>
                {/if}
              </div>

              <!-- Info -->
              <div
                class={cn(
                  "flex min-w-0 flex-1 flex-col gap-0.5",
                  view === "grid" && "px-3 py-2.5",
                )}
              >
                <div
                  class="truncate text-[12.5px] font-semibold text-foreground"
                >
                  {entry.filename}
                </div>
                <div class="truncate text-[10.5px] text-muted-foreground/80">
                  {formatSize(entry.sizeBytes)} · {libraryDate(entry.created)}
                </div>
              </div>

              <!-- Actions -->
              {#if !selection.selectMode}
                <div
                  role="presentation"
                  onclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => e.stopPropagation()}
                  class={view === "grid"
                    ? "absolute right-2 top-2"
                    : "shrink-0 pr-1"}
                >
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                      {#snippet child({ props })}
                        <Button
                          {...props as Record<string, unknown>}
                          variant="ghost"
                          size="icon-sm"
                          class={cn(
                            "size-7 opacity-0 transition-opacity duration-200 group-hover/card:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100",
                            view === "grid" &&
                              "border border-border/60 bg-background/80 text-foreground/70 backdrop-blur-md hover:bg-background hover:text-foreground",
                          )}
                          title="More actions"
                        >
                          <MoreHorizontal size={14} />
                        </Button>
                      {/snippet}
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content align="end" size="sm" class="w-50">
                      <DropdownMenu.Item
                        onSelect={() => openFileLocation(entry.path)}
                      >
                        <FolderOpen /> Show in folder
                      </DropdownMenu.Item>
                      <DropdownMenu.Item
                        onSelect={() => (renameTarget = entry)}
                      >
                        <Pencil /> Rename…
                      </DropdownMenu.Item>
                      <DropdownMenu.Item onSelect={() => copyPath(entry)}>
                        <CopyIcon /> Copy path
                      </DropdownMenu.Item>
                      {#if shareSupported}
                        <DropdownMenu.Item onSelect={() => shareEntry(entry)}>
                          <ShareIcon /> Share…
                        </DropdownMenu.Item>
                      {/if}
                      <DropdownMenu.Separator />
                      {#if gdrive.uploadHistory[entry.path]}
                        <!-- No "re-upload" action by design: it would mint a new
                             Drive file (new fileId) and abandon the URL the user
                             already shared. "Forget" is the way back to upload. -->
                        <DropdownMenu.Item
                          onSelect={() => copyDriveLink(entry)}
                        >
                          <Link2 /> Copy Drive link
                        </DropdownMenu.Item>
                        <DropdownMenu.Item
                          onSelect={() => openDriveLink(entry)}
                        >
                          <ExternalLink /> Open in Drive
                        </DropdownMenu.Item>
                        <DropdownMenu.Separator />
                        <DropdownMenu.Item
                          onSelect={() => forgetDriveLink(entry)}
                          class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                        >
                          <Unlink2 /> Forget Drive link
                        </DropdownMenu.Item>
                      {:else}
                        <DropdownMenu.Item
                          onSelect={() => uploadToDrive(entry)}
                        >
                          <BrandGoogleDrive /> Upload to Drive
                        </DropdownMenu.Item>
                      {/if}
                      <DropdownMenu.Separator />
                      {#if cloudShare.uploadHistory[entry.path]}
                        <!-- "Manage" opens scope/password/expiry/delete; "Forget"
                             drops only the local association. -->
                        <DropdownMenu.Item onSelect={() => copyCloudLink(entry)}>
                          <Link2 /> Copy share link
                        </DropdownMenu.Item>
                        <DropdownMenu.Item onSelect={() => openCloudLink(entry)}>
                          <ExternalLink /> Open share page
                        </DropdownMenu.Item>
                        <DropdownMenu.Item onSelect={() => (manageTarget = entry)}>
                          <SlidersHorizontal /> Manage share…
                        </DropdownMenu.Item>
                        <DropdownMenu.Separator />
                        <DropdownMenu.Item
                          onSelect={() => forgetCloudShare(entry)}
                          class="text-destructive focus:bg-destructive/10 focus:text-destructive"
                        >
                          <Unlink2 /> Forget cloud link
                        </DropdownMenu.Item>
                      {:else}
                        <DropdownMenu.Item onSelect={() => shareToCloud(entry)} class="whitespace-nowrap">
                          <RecastMark /> Share to Recast Cloud
                        </DropdownMenu.Item>
                      {/if}
                      <DropdownMenu.Separator />
                      <DropdownMenu.Item
                        onSelect={() => (deleteTarget = entry)}
                        class="text-destructive focus:bg-destructive/10 focus:text-destructive"
                      >
                        <Trash2 /> Move to trash
                      </DropdownMenu.Item>
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                </div>
              {/if}

            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>

<!-- Floating bulk-action bar, visible whenever selection mode is on. -->
{#if selection.selectMode}
  <div
    in:fly={{ y: 24, duration: 220, easing: cubicOut }}
    out:fly={{ y: 24, duration: 160, easing: cubicOut }}
    class="fixed inset-x-0 bottom-6 z-40 flex justify-center px-6"
  >
    <div
      class="flex items-center gap-1.5 rounded-2xl border border-border bg-card/95 p-1.5 px-5 shadow-2xl ring-1 ring-border/40 backdrop-blur-xl"
    >
      <span class="text-[12px] font-medium tabular-nums text-foreground">
        {selectedCount} selected
      </span>
      <div class="mx-1 h-4 w-px bg-border/60"></div>
      <Button
        variant="ghost"
        size="xs"
        onclick={() => selection.toggleAll(filtered)}
        disabled={filtered.length === 0}
      >
        {allFilteredSelected ? "Clear all" : "Select all"}
      </Button>
      <Button
        variant="destructive"
        size="xs"
        onclick={() => (bulkDeleteOpen = true)}
        disabled={selectedCount === 0}
      >
        <Trash2 size={12} />
        Delete{selectedCount > 0 ? ` (${selectedCount})` : ""}
      </Button>
      <Button
        variant="secondary"
        size="xs"
        onclick={selection.exit}
      >
        Cancel
      </Button>
    </div>
  </div>
{/if}

{#if bulkDeleteOpen}
  <ConfirmDialog
    open={true}
    title={`Move ${selectedCount} export${selectedCount === 1 ? "" : "s"} to trash?`}
    description="The selected exports will be sent to the recycle bin. You can restore them from there if needed."
    confirmLabel="Move to trash"
    variant="destructive"
    onConfirm={selection.bulkDelete}
    onOpenChange={(v) => {
      if (!v) bulkDeleteOpen = false;
    }}
  />
{/if}

{#if renameTarget}
  <RenameDialog
    open={true}
    title="Rename export"
    label="New filename"
    initialValue={renameTarget.filename}
    onSave={async (next) => {
      await handleRename(renameTarget!, next);
    }}
    onOpenChange={(v) => {
      if (!v) renameTarget = null;
    }}
  />
{/if}

{#if deleteTarget}
  <ConfirmDialog
    open={true}
    title="Move export to trash?"
    description={`“${deleteTarget.filename}” will be sent to the recycle bin. You can restore it from there if needed.`}
    confirmLabel="Move to trash"
    variant="destructive"
    onConfirm={async () => {
      await handleDelete(deleteTarget!);
    }}
    onOpenChange={(v) => {
      if (!v) deleteTarget = null;
    }}
  />
{/if}

{#if playTarget}
  <PlayerDialog entry={playTarget} onclose={() => (playTarget = null)} />
{/if}

{#if manageTarget && cloudShare.uploadHistory[manageTarget.path]}
  <ShareManageDialog
    open={true}
    record={cloudShare.uploadHistory[manageTarget.path]}
    fileName={manageTarget.filename}
    path={manageTarget.path}
    onOpenChange={(v: boolean) => {
      if (!v) manageTarget = null;
    }}
  />
{/if}

{#if workspacePick}
  <WorkspacePickerDialog
    open={true}
    workspaces={cloudShare.workspaces}
    activeId={cloudShare.activeWorkspaceId}
    fileName={workspacePick.fileName}
    onConfirm={(workspaceId, remember) => {
      const pick = workspacePick;
      if (!pick) return;
      if (remember) cloudShare.setWorkspace(workspaceId);
      beginCloudShare(pick.path, pick.title, workspaceId);
    }}
    onOpenChange={(v: boolean) => {
      if (!v) workspacePick = null;
    }}
  />
{/if}
