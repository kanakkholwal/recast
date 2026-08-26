<script lang="ts">
import { goto } from "$app/navigation";
import ShareManageDialog from "$components/cloud/ShareManageDialog.svelte";
import WorkspacePickerDialog from "$components/cloud/WorkspacePickerDialog.svelte";
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
import StudioPage from "$components/layout/StudioPage.svelte";
import { ConfirmDialog, PlayerDialog, RenameDialog } from "$components/recast";
import RecastMark from "$components/recast-mark.svelte";
import { listExports, openFileLocation, type RecordingEntry } from "$lib/ipc";
import { cardShellClass, listClass } from "$lib/library/card-styles";
import { createLibraryPage } from "$lib/library/library-page.svelte";
import { canReportCount } from "$lib/library/status";
import { morph } from "$lib/morph";
import { isShareSupported, shareRecording } from "$lib/share";
import { shareTargetFor } from "$lib/share-target";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import { gdrive } from "$lib/stores/gdrive.svelte";
import { formatSize, getExtension } from "@recast/editor/lib/format/files";
import { motionDuration } from "@recast/editor/lib/motion.svelte";
import {
	BrandGoogleDrive,
	CopyIcon,
	Download,
	ExternalLink,
	FolderOpen,
	Link2,
	ListChecks,
	MoreHorizontal,
	Pencil,
	RefreshCw,
	SlidersHorizontal,
	Trash2,
	Unlink2,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { toast } from "@recast/ui/sonner";
import { cn } from "@recast/ui/utils";
import { platform } from "@tauri-apps/plugin-os";
import { onMount } from "svelte";
import { settingsHref } from "../settings/settings-tabs";

const lib = createLibraryPage({
	noun: "export",
	viewKey: "exports-view",
	load: listExports,
	matchExtension: true,
	onEntryRemoved: (entry) => {
		// Local file is gone: drop upload records so the row doesn't return next
		// session claiming a copy. Remote objects are left untouched.
		void gdrive.forgetUpload(entry.path);
		void cloudShare.forget(entry.path);
	},
});

let renameTarget = $state<RecordingEntry | null>(null);
let deleteTarget = $state<RecordingEntry | null>(null);
let manageTarget = $state<RecordingEntry | null>(null);
let playTarget = $state<RecordingEntry | null>(null);
let workspacePick = $state<{ path: string; title: string; fileName: string } | null>(null);
let bulkDeleteOpen = $state(false);

const shareSupported = isShareSupported();
const ShareIcon = shareTargetFor(platform()).icon;

const titleText = $derived(
	!canReportCount(lib.status)
		? "Exports"
		: lib.entries.length === 0
			? "Nothing exported yet"
			: lib.entries.length === 1
				? "1 export"
				: `${lib.entries.length} exports`,
);
const subtitleText = $derived(
	canReportCount(lib.status)
		? `${formatSize(lib.totalSize)} on disk`
		: "Rendered videos, ready to share",
);

onMount(() => {
	lib.refresh();
	lib.restoreView();
	// Hydrate upload history so each row's menu picks the right action.
	void gdrive.init();
	void cloudShare.init();
});

function activateEntry(entry: RecordingEntry) {
	if (lib.selection.selectMode) lib.selection.toggle(entry.path);
	else playTarget = entry;
}

async function shareToCloud(entry: RecordingEntry) {
	if (!cloudShare.initialized) {
		const tid = toast.loading("Connecting to Recast Cloud…");
		await cloudShare.init();
		toast.dismiss(tid);
	}
	if (!cloudShare.signedIn) {
		toast.info("Sign in to Recast Cloud in Settings first.");
		void goto(settingsHref("cloud"));
		return;
	}
	const title = entry.filename.replace(/\.[^.]+$/, "");
	if (cloudShare.workspaces.length > 1) {
		workspacePick = { path: entry.path, title, fileName: entry.filename };
		void cloudShare.refreshStatus();
		return;
	}
	beginCloudShare(entry.path, title);
}

function beginCloudShare(path: string, title: string, workspaceId: string | undefined = undefined) {
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

async function uploadToDrive(entry: RecordingEntry) {
	await gdrive.init();
	if (!gdrive.connected) {
		toast.info("Connect Google Drive in Settings first.");
		void goto(settingsHref("cloud"));
		return;
	}
	const id = gdrive.startUpload(entry.path);
	requestAnimationFrame(() => gdrive.setForeground(id));
}

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

async function forgetDriveLink(entry: RecordingEntry) {
	await gdrive.forgetUpload(entry.path);
	toast.success(`Forgot Drive link for "${entry.filename}"`);
}
</script>

<StudioPage title={titleText} subtitle={subtitleText}>
  {#snippet filters()}
    <div class="min-w-[180px] flex-1">
      <LibrarySearch bind:value={lib.query} noun="exports" />
    </div>
    <Button
      variant="ghost"
      size="sm"
      class={cn(
        "h-9 gap-1.5 rounded-lg px-3 text-[12px]",
        lib.selection.selectMode
          ? "bg-foreground text-background hover:bg-foreground/90 hover:text-background"
          : "text-muted-foreground hover:text-foreground",
      )}
      onclick={lib.selection.toggleMode}
      disabled={lib.entries.length === 0}
      aria-pressed={lib.selection.selectMode}
      title="Select multiple exports"
    >
      <ListChecks size={12} />
      {lib.selection.selectMode ? "Done" : "Select"}
    </Button>
    <LibrarySortSelect bind:value={lib.sort} noun="exports" />
    <LibraryViewToggle bind:value={lib.view} />
    <Button
      variant="ghost"
      size="icon-sm"
      class="size-9 rounded-lg text-muted-foreground hover:text-foreground"
      onclick={lib.refresh}
      disabled={lib.isLoading}
      aria-label="Refresh exports"
      title="Refresh"
    >
      <RefreshCw size={12} class={lib.isLoading ? "motion-safe:animate-spin" : ""} />
    </Button>
  {/snippet}

  {#if lib.status === "loading"}
    <LibrarySkeletonGrid view={lib.view} />
  {:else if lib.status === "error"}
    <LibraryError
      title="Couldn't load your exports"
      message={lib.loadError ?? "Unknown error"}
      onRetry={lib.refresh}
    />
  {:else if lib.status === "empty" || lib.status === "no-matches"}
    <LibraryEmpty
      icon={Download}
      title={lib.query ? "No matches" : "Nothing exported yet"}
      description={lib.query
        ? `Nothing matches "${lib.query}".`
        : "Render a recording from the editor and it'll show up here."}
    />
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
            placeholderIcon={Download}
            typeLabel={getExtension(entry.filename)}
            onOpen={() => activateEntry(entry)}
          >
            {#snippet footer()}
              {@const cloudRec = cloudShare.getRecordForPath(entry.path)}
              {@const driveRec = gdrive.getRecordForPath(entry.path)}
              {#if cloudRec || driveRec}
                <span class="mt-1 flex flex-wrap items-center gap-1">
                  {#if cloudRec}
                    <span
                      class="inline-flex items-center gap-1 rounded bg-muted/60 px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground ring-1 ring-inset ring-border/40"
                      title="Shared to Recast Cloud"
                    >
                      <RecastMark class="size-2.5" /> Cloud
                    </span>
                  {/if}
                  {#if driveRec}
                    <span
                      class="inline-flex items-center gap-1 rounded bg-muted/60 px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground ring-1 ring-inset ring-border/40"
                      title="Uploaded to Google Drive"
                    >
                      <BrandGoogleDrive class="size-2.5" /> Drive
                    </span>
                  {/if}
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
                <DropdownMenu.Content align="end" size="sm" class="w-50">
                  <DropdownMenu.Item onSelect={() => openFileLocation(entry.path)}>
                    <FolderOpen /> Show in folder
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => (renameTarget = entry)}>
                    <Pencil /> Rename…
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => lib.copyPath(entry)}>
                    <CopyIcon /> Copy path
                  </DropdownMenu.Item>
                  {#if shareSupported}
                    <DropdownMenu.Item onSelect={() => shareEntry(entry)}>
                      <ShareIcon /> Share…
                    </DropdownMenu.Item>
                  {/if}
                  <DropdownMenu.Separator />
                  {#if gdrive.uploadHistory[entry.path]}
                    <DropdownMenu.Item onSelect={() => copyDriveLink(entry)}>
                      <Link2 /> Copy Drive link
                    </DropdownMenu.Item>
                    <DropdownMenu.Item onSelect={() => openDriveLink(entry)}>
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
                    <DropdownMenu.Item onSelect={() => uploadToDrive(entry)}>
                      <BrandGoogleDrive /> Upload to Drive
                    </DropdownMenu.Item>
                  {/if}
                  <DropdownMenu.Separator />
                  {#if cloudShare.uploadHistory[entry.path]}
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

{#if bulkDeleteOpen}
  <ConfirmDialog
    open={true}
    title={`Move ${lib.selectedCount} export${lib.selectedCount === 1 ? "" : "s"} to trash?`}
    description="The selected exports will be sent to the recycle bin. You can restore them from there if needed."
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
    title="Rename export"
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
    title="Move export to trash?"
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
