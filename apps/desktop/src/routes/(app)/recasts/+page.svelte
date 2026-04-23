<script lang="ts">
  import { goto } from "$app/navigation";
  import {
    ConfirmDialog,
    RecastList,
    RenameDialog,
    type RecastListItem,
  } from "$components/recast";
  import {
    deleteFile,
    generateThumbnails,
    listRecasts,
    openFileLocation,
    renameFile,
    type RecordingEntry,
  } from "$lib/ipc";
  import {
    CopyIcon,
    ExternalLink,
    Film,
    FolderOpen,
    Pencil,
    RefreshCw,
    Trash2
  } from "@lucide/svelte";

  import { Button } from "@recast/ui/button";
  import { toast } from "@recast/ui/sonner";
  import { listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";

  let entries = $state<RecordingEntry[]>([]);
  let isLoading = $state(true);
  let thumbnails = $state<Record<string, string>>({});
  let editorWindow = $state<"navigate" | "new-window">("navigate");
  let thumbnailPass = 0;

  // Dialog state — shared across all rows via current target
  let renameTarget = $state<RecordingEntry | null>(null);
  let deleteTarget = $state<RecordingEntry | null>(null);

  onMount(() => {
    fetchRecasts();
    const stored = localStorage.getItem("recast-editor-window") as "navigate" | "new-window" | null;
    if (stored) editorWindow = stored;
    const unlisten = listen("refresh-recordings", () => fetchRecasts());
    return () => { unlisten.then((fn) => fn()); };
  });

  async function fetchRecasts() {
    isLoading = true;
    try {
      entries = await listRecasts();
      loadThumbnails(entries);
    } catch (e) {
      toast.error(`Could not load recordings: ${e}`);
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

  function encodeEditorPath(path: string) {
    return encodeURIComponent(btoa(encodeURIComponent(path)));
  }

  async function openInEditor(entry: RecordingEntry) {
    const route = `/editor/${encodeEditorPath(entry.path)}`;
    if (editorWindow === "new-window") {
      await openInNewWindow(entry);
    } else {
      goto(route);
    }
    void route;
  }

  async function openInNewWindow(entry: RecordingEntry) {
    const route = `/editor/${encodeEditorPath(entry.path)}`;
    const label = `editor-${encodeEditorPath(entry.path)
      .replace(/[^a-zA-Z0-9]/g, "")
      .slice(0, 48)}`;
    const existing = await WebviewWindow.getByLabel(label);
    if (existing) {
      await existing.setFocus();
      return;
    }
    new WebviewWindow(label, {
      url: route,
      title: `Editor - ${entry.filename}`,
      width: 1440,
      height: 960,
      center: true,
      decorations: false,
    });
  }

  async function handleRename(entry: RecordingEntry, nextName: string) {
    // Let the Rust side enforce the hard validation — we only trim here.
    const newPath = await renameFile(entry.path, nextName);
    entries = entries.map((e) =>
      e.path === entry.path
        ? { ...e, path: newPath, filename: newPath.split(/[\\/]/).pop() ?? nextName }
        : e,
    );
    // Move thumbnail reference to the new path so the grid doesn't flash.
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
    // Drop the thumbnail we no longer need.
    if (thumbnails[entry.path]) {
      const { [entry.path]: _, ...rest } = thumbnails;
      thumbnails = rest;
    }
    toast.success(`Moved "${entry.filename}" to trash`);
  }

  async function copyPath(entry: RecordingEntry) {
    try {
      await navigator.clipboard.writeText(entry.path);
      toast.success("Path copied to clipboard");
    } catch (e) {
      toast.error(`Copy failed: ${e}`);
    }
  }

  const items = $derived<RecastListItem[]>(
    entries.map((entry) => ({
      id: entry.path,
      title: entry.filename,
      subtitle: `${formatSize(entry.sizeBytes)} · ${formatDate(entry.created)}`,
      icon: Film,
      iconImage: thumbnails[entry.path],
      keywords: [entry.filename, "recording", "recast"],
      accessories: [{ text: ".recast", variant: "info" }],
      onSelect: () => openInEditor(entry),
      actions: [
        {
          id: "open",
          label: "Open in Editor",
          icon: Pencil,
          onAction: () => openInEditor(entry),
        },
        {
          id: "open-new-window",
          label: "Open in New Window",
          icon: ExternalLink,
          shortcut: "⌘↵",
          onAction: () => openInNewWindow(entry),
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
          id: "show",
          label: "Show in Folder",
          icon: FolderOpen,
          onAction: () => openFileLocation(entry.path),
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
  title="Recasts"
  subtitle="Your screen recordings as .recast projects"
  searchPlaceholder="Search recordings..."
  emptyTitle="No recordings yet"
  emptyHint="Take your first recording from the Recast Panel."
>
  {#snippet toolbar()}
    <Button variant="ghost" size="icon-sm" onclick={fetchRecasts} disabled={isLoading} title="Refresh">
      <RefreshCw size={14} class={isLoading ? "animate-spin" : ""} />
    </Button>
  {/snippet}
</RecastList>


{#if renameTarget}
  <RenameDialog
    open={true}
    title="Rename recording"
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
    title="Move recording to trash?"
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
