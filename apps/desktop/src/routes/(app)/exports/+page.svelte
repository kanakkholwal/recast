<script lang="ts">
  import {
    ConfirmDialog,
    RecastList,
    RenameDialog,
    type RecastListItem,
  } from "$components/recast";
  import {
    deleteFile,
    generateThumbnails,
    listExports,
    openFileLocation,
    renameFile,
    type RecordingEntry,
  } from "$lib/ipc";

  import { CopyIcon, FolderOpen, Pencil, Play, RefreshCw, Trash2 } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { toast } from "@recast/ui/sonner";
  import { onMount } from "svelte";

  let entries = $state<RecordingEntry[]>([]);
  let isLoading = $state(true);
  let thumbnails = $state<Record<string, string>>({});
  let thumbnailPass = 0;

  let renameTarget = $state<RecordingEntry | null>(null);
  let deleteTarget = $state<RecordingEntry | null>(null);

  onMount(() => {
    fetchExports();
  });

  async function fetchExports() {
    isLoading = true;
    try {
      entries = await listExports();
      loadThumbnails(entries);
    } catch (e) {
      toast.error(`Could not load exports: ${e}`);
    } finally {
      isLoading = false;
    }
  }

  async function loadThumbnails(items: RecordingEntry[]) {
    const pass = ++thumbnailPass;
    const settled = await Promise.allSettled(
      items.map(async (item) => {
        const frames = await generateThumbnails(item.path, 1);
        return [item.path, frames[0] ?? ""] as const;
      }),
    );
    if (pass !== thumbnailPass) return;
    const next: Record<string, string> = {};
    for (const r of settled) {
      if (r.status === "fulfilled" && r.value[1]) next[r.value[0]] = r.value[1];
    }
    thumbnails = next;
  }

  function formatSize(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  }

  function formatDate(unix: number) {
    return new Date(unix * 1000).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function getExtension(filename: string) {
    const dot = filename.lastIndexOf(".");
    return dot >= 0 ? filename.slice(dot + 1).toUpperCase() : "FILE";
  }

  async function copyPath(entry: RecordingEntry) {
    try {
      await navigator.clipboard.writeText(entry.path);
      toast.success("Path copied to clipboard");
    } catch (e) {
      toast.error(`Copy failed: ${e}`);
    }
  }

  async function handleRename(entry: RecordingEntry, nextName: string) {
    const newPath = await renameFile(entry.path, nextName);
    entries = entries.map((e) =>
      e.path === entry.path
        ? { ...e, path: newPath, filename: newPath.split(/[\\/]/).pop() ?? nextName }
        : e,
    );
    const existingThumb = thumbnails[entry.path];
    if (existingThumb) {
      const { [entry.path]: _, ...rest } = thumbnails;
      thumbnails = { ...rest, [newPath]: existingThumb };
    }
    toast.success("Renamed");
  }

  async function handleDelete(entry: RecordingEntry) {
    await deleteFile(entry.path);
    entries = entries.filter((e) => e.path !== entry.path);
    if (thumbnails[entry.path]) {
      const { [entry.path]: _, ...rest } = thumbnails;
      thumbnails = rest;
    }
    toast.success(`Moved "${entry.filename}" to trash`);
  }

  const items = $derived<RecastListItem[]>(
    entries.map((entry) => ({
      id: entry.path,
      title: entry.filename,
      subtitle: `${formatSize(entry.sizeBytes)} · ${formatDate(entry.created)}`,
      icon: Play,
      iconImage: thumbnails[entry.path],
      keywords: [entry.filename, "export", getExtension(entry.filename).toLowerCase()],
      accessories: [{ text: getExtension(entry.filename), variant: "default" }],
      onSelect: () => openFileLocation(entry.path),
      actions: [
        {
          id: "show",
          label: "Show in Folder",
          icon: FolderOpen,
          shortcut: "⌘O",
          onAction: () => openFileLocation(entry.path),
        },
        {
          id: "rename",
          label: "Rename…",
          icon: Pencil,
          shortcut: "⌘R",
          onAction: () => {
            renameTarget = entry;
          },
        },
        {
          id: "copy-path",
          label: "Copy Path",
          shortcut: "⌘⇧C",
                    icon:CopyIcon,

          onAction: () => copyPath(entry),
        },
        {
          id: "delete",
          label: "Move to Trash",
          icon: Trash2,
          variant: "destructive",
          shortcut: "⌘⌫",
          onAction: () => {
            deleteTarget = entry;
          },
        },
      ],
    })),
  );
</script>

<RecastList
  {items}
  {isLoading}
  title="Exports"
  subtitle="Exported videos ready to share"
  searchPlaceholder="Search exports..."
  emptyTitle="No exports yet"
  emptyHint="Export a recording from the editor to see it here."
>
  {#snippet toolbar()}
    <Button variant="ghost" size="icon-sm" onclick={fetchExports} disabled={isLoading} title="Refresh">
      <RefreshCw size={14} class={isLoading ? "animate-spin" : ""} />
    </Button>
  {/snippet}
</RecastList>

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
    confirmLabel="Move to Trash"
    variant="destructive"
    onConfirm={async () => {
      await handleDelete(deleteTarget!);
    }}
    onOpenChange={(v) => {
      if (!v) deleteTarget = null;
    }}
  />
{/if}
