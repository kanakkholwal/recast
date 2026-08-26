<script lang="ts">
import StudioPage from "$components/layout/StudioPage.svelte";
import {
	AssetCard,
	LibraryEmpty,
	LibraryError,
	LibrarySearch,
	LibrarySkeletonGrid,
	LibrarySortSelect,
	LibraryViewToggle,
	SelectionBar,
} from "$components/library";
import { ConfirmDialog, RenameDialog } from "$components/recast";
import {
	launchRecordingPanel,
	listRecasts,
	migrateProject,
	openFileLocation,
	type RecordingEntry,
} from "$lib/ipc";
import { cardShellClass, listClass } from "$lib/library/card-styles";
import { openInEditor as openEditorWindow, openInNewWindow } from "$lib/library/editor-window";
import { createLibraryPage } from "$lib/library/library-page.svelte";
import { canReportCount } from "$lib/library/status";
import { morph } from "$lib/morph";
import { isShareSupported, shareRecording } from "$lib/share";
import { shareTargetFor } from "$lib/share-target";
import { formatSize } from "@recast/editor/lib/format/files";
import { motionDuration } from "@recast/editor/lib/motion.svelte";
import {
	CopyIcon,
	ExternalLink,
	Film,
	FolderOpen,
	History,
	ListChecks,
	MoreHorizontal,
	Pencil,
	RefreshCw,
	Trash2,
	Video,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { safeStorage } from "@recast/ui/persisted-state";
import { toast } from "@recast/ui/sonner";
import { cn } from "@recast/ui/utils";
import { listen } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import { onMount } from "svelte";

const lib = createLibraryPage({ noun: "recording", viewKey: "recasts-view", load: listRecasts });

let editorWindow = $state<"navigate" | "new-window">("navigate");
let renameTarget = $state<RecordingEntry | null>(null);
let deleteTarget = $state<RecordingEntry | null>(null);
let bulkDeleteOpen = $state(false);
let migrateAllOpen = $state(false);
let migrating = $state(false);
const legacyCount = $derived(lib.entries.filter((e) => e.needsMigration).length);

const shareSupported = isShareSupported();
const ShareIcon = shareTargetFor(platform()).icon;

const titleText = $derived(
	!canReportCount(lib.status)
		? "Recordings"
		: lib.entries.length === 0
			? "No recordings yet"
			: lib.entries.length === 1
				? "1 recording"
				: `${lib.entries.length} recordings`,
);
const subtitleText = $derived(
	canReportCount(lib.status)
		? `${formatSize(lib.totalSize)} on disk`
		: "Your screen recordings, ready to edit",
);

onMount(() => {
	lib.refresh();
	lib.restoreView();
	editorWindow = safeStorage.get<"navigate" | "new-window">("recast-editor-window", editorWindow);
	const unlisten = listen("refresh-recordings", () => lib.refresh());
	return () => unlisten.then((fn) => fn());
});

const openInEditor = (entry: RecordingEntry) => openEditorWindow(entry, editorWindow);

function activateEntry(entry: RecordingEntry) {
	if (lib.selection.selectMode) lib.selection.toggle(entry.path);
	else openInEditor(entry);
}

async function newRecording() {
	try {
		await launchRecordingPanel();
	} catch (e) {
		toast.error(`Couldn't open the recorder: ${e}`);
	}
}

async function shareEntry(entry: RecordingEntry) {
	const result = await shareRecording({
		path: entry.path,
		fileName: entry.filename,
		title: entry.filename,
		text: "Recorded with Recast",
	});
	if (result.ok || result.reason === "cancelled") return;
	if (result.reason === "unsupported") toast.error("Sharing files isn't available on this device.");
	else toast.error(`Share failed: ${result.message ?? "unknown error"}`);
}

async function handleMigrateAll() {
	const legacy = lib.entries.filter((e) => e.needsMigration);
	migrating = true;
	let ok = 0;
	for (const e of legacy) {
		try {
			await migrateProject(e.path);
			ok++;
		} catch (err) {
			console.warn("Migration failed:", e.path, err);
		}
	}
	migrating = false;
	const failed = legacy.length - ok;
	if (failed > 0) toast.error(`Updated ${ok} · ${failed} failed`);
	else toast.success(`Updated ${ok} project${ok === 1 ? "" : "s"}`);
	await lib.refresh();
}

async function handleMigrateOne(entry: RecordingEntry) {
	try {
		await migrateProject(entry.path);
		toast.success(`Updated "${entry.filename}"`);
		await lib.refresh();
	} catch (err) {
		toast.error(`Update failed: ${err}`);
	}
}
</script>

<StudioPage title={titleText} subtitle={subtitleText}>
  {#snippet actions()}
    {#if legacyCount > 0}
      <Button
        variant="secondary"
        size="sm"
        class="gap-1.5"
        onclick={() => (migrateAllOpen = true)}
        disabled={migrating}
        title="Update older projects to the current format"
      >
        {#if migrating}
          <RefreshCw size={13} class="motion-safe:animate-spin" />
        {:else}
          <History size={13} />
        {/if}
        Update {legacyCount} older
      </Button>
    {/if}
    <Button size="sm" class="gap-1.5" variant="outline" onclick={newRecording}>
      <Video class="size-4" /> New recording
    </Button>
  {/snippet}

  {#snippet filters()}
    <div class="w-full max-w-md flex-1">
      <LibrarySearch bind:value={lib.query} noun="recordings" />
    </div>
    <Button
      variant={lib.selection.selectMode ? "default_soft" : "ghost"}
      size="sm"
      class={cn(
        "ml-auto",
       
      )}
      onclick={lib.selection.toggleMode}
      disabled={lib.entries.length === 0}
      aria-pressed={lib.selection.selectMode}
      title="Select multiple recordings"
    >
      <ListChecks size={12} />
      {lib.selection.selectMode ? "Done" : "Select"}
    </Button>
    <LibrarySortSelect bind:value={lib.sort} noun="recordings" />
    <LibraryViewToggle bind:value={lib.view} />
    <Button
      variant="ghost"
      size="icon-sm"
      onclick={lib.refresh}
      disabled={lib.isLoading}
      aria-label="Refresh recordings"
      title="Refresh"
    >
      <RefreshCw size={12} class={lib.isLoading ? "motion-safe:animate-spin" : "group-active/button:rotate-90 duration-500"} />
    </Button>
  {/snippet}

  {#if lib.status === "loading"}
    <LibrarySkeletonGrid view={lib.view} />
  {:else if lib.status === "error"}
    <LibraryError
      title="Couldn't load your recordings"
      message={lib.loadError ?? "Unknown error"}
      onRetry={lib.refresh}
    />
  {:else if lib.status === "empty" || lib.status === "no-matches"}
    <LibraryEmpty
      icon={Film}
      title={lib.query ? "No matches" : "No recordings yet"}
      description={lib.query
        ? `Nothing matches "${lib.query}".`
        : "Record your screen and it lands here, ready to edit."}
    >
      {#snippet action()}
        {#if lib.query}
          <Button variant="secondary" size="sm" onclick={() => (lib.query = "")}>Clear search</Button>
        {:else}
          <Button class="gap-2" onclick={newRecording}>
            <Video class="size-4" /> Start recording
          </Button>
        {/if}
      {/snippet}
    </LibraryEmpty>
  {:else}
    <div class={listClass(lib.view)}>
      {#each lib.displayed as entry (entry.path)}
        {@const isSelected = lib.selection.has(entry.path)}
        <div
          animate:morph={{ duration: motionDuration(340) }}
          title={entry.filename}
          class={cardShellClass(lib.view, isSelected)}
        >
          <AssetCard
            entry={entry}
            thumbnail={lib.thumbnails[entry.path]}
            view={lib.view}
            selectMode={lib.selection.selectMode}
            selected={isSelected}
            placeholderIcon={Film}
            typeLabel=".recast"
            onOpen={() => activateEntry(entry)}
          >
            {#snippet footer()}
              {#if entry.needsMigration}
                <span
                  class="mt-1 inline-flex w-fit items-center gap-1 rounded bg-warning/10 px-1.5 py-0.5 text-[9px] font-medium text-warning"
                  title="Older project format. Update to keep editing."
                >
                  <History size={9} /> Older format
                </span>
              {/if}
            {/snippet}
            {#snippet actions()}
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button
                      {...props as Record<string, unknown>}
                      variant="ghost"
                      size="icon-sm"
                      class={cn(
                        "size-7 opacity-0 transition-opacity duration-200 group-hover/card:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100",
                        lib.view === "grid" &&
                          "border border-border/60 bg-background/80 text-foreground/70 backdrop-blur-md hover:bg-background hover:text-foreground",
                      )}
                      title="More actions"
                    >
                      <MoreHorizontal size={14} />
                    </Button>
                  {/snippet}
                </DropdownMenu.Trigger>
                <DropdownMenu.Content align="end" size="sm" class="w-44">
                  {#if entry.needsMigration}
                    <DropdownMenu.Item onSelect={() => handleMigrateOne(entry)}>
                      <History class="size-3" /> Update format
                    </DropdownMenu.Item>
                    <DropdownMenu.Separator />
                  {/if}
                  <DropdownMenu.Item onSelect={() => openInEditor(entry)}>
                    <Pencil class="size-3" /> Open in editor
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => openInNewWindow(entry)}>
                    <ExternalLink class="size-3" /> New window
                  </DropdownMenu.Item>
                  <DropdownMenu.Separator />
                  <DropdownMenu.Item onSelect={() => (renameTarget = entry)}>
                    <Pencil class="size-3" /> Rename…
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => openFileLocation(entry.path)}>
                    <FolderOpen class="size-3" /> Show in folder
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => lib.copyPath(entry)}>
                    <CopyIcon class="size-3" /> Copy path
                  </DropdownMenu.Item>
                  {#if shareSupported}
                    <DropdownMenu.Item onSelect={() => shareEntry(entry)}>
                      <ShareIcon class="size-3" /> Share…
                    </DropdownMenu.Item>
                  {/if}
                  <DropdownMenu.Separator />
                  <DropdownMenu.Item
                    onSelect={() => (deleteTarget = entry)}
                    class="text-destructive focus:bg-destructive/10 focus:text-destructive"
                  >
                    <Trash2 class="size-3" /> Move to trash
                  </DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            {/snippet}
          </AssetCard>
        </div>
      {/each}
    </div>
  {/if}
</StudioPage>

{#if lib.selection.selectMode}
  <SelectionBar
    count={lib.selectedCount}
    allSelected={lib.allFilteredSelected}
    canSelectAll={lib.filtered.length > 0}
    onToggleAll={() => lib.selection.toggleAll(lib.filtered)}
    onDelete={() => (bulkDeleteOpen = true)}
    onCancel={lib.selection.exit}
  />
{/if}

{#if migrateAllOpen}
  <ConfirmDialog
    open={true}
    title={`Update ${legacyCount} older project${legacyCount === 1 ? "" : "s"}?`}
    description="These projects use an older format. Each is updated in place to the current format, keeping a backup (.bak) next to it. You can also update them one at a time from a project's menu."
    confirmLabel="Update all"
    onConfirm={handleMigrateAll}
    onOpenChange={(v) => {
      if (!v) migrateAllOpen = false;
    }}
  />
{/if}

{#if bulkDeleteOpen}
  <ConfirmDialog
    open={true}
    title={`Move ${lib.selectedCount} recording${lib.selectedCount === 1 ? "" : "s"} to trash?`}
    description="The selected recordings will be sent to the recycle bin. You can restore them from there if needed."
    confirmLabel="Move to trash"
    variant="destructive"
    onConfirm={lib.selection.bulkDelete}
    onOpenChange={(v) => {
      if (!v) bulkDeleteOpen = false;
    }}
  />
{/if}

{#if renameTarget}
  <RenameDialog
    open={true}
    title="Rename recording"
    label="New filename"
    initialValue={renameTarget.filename}
    onSave={async (next) => {
      await lib.handleRename(renameTarget!, next);
    }}
    onOpenChange={(v) => {
      if (!v) renameTarget = null;
    }}
  />
{/if}

{#if deleteTarget}
  <ConfirmDialog
    open={true}
    title="Move recording to trash?"
    description={`“${deleteTarget.filename}” will be sent to the recycle bin. You can restore it from there if needed.`}
    confirmLabel="Move to trash"
    variant="destructive"
    onConfirm={async () => {
      await lib.handleDelete(deleteTarget!);
    }}
    onOpenChange={(v) => {
      if (!v) deleteTarget = null;
    }}
  />
{/if}
