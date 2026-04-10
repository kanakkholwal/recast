<script lang="ts">
  import { RaycastList, type RaycastListItem } from "$components/raycast";
  import { Button } from "$components/ui/button";
  import { generateThumbnails, listExports, openFileLocation, type RecordingEntry } from "$lib/ipc";
  import { Download, FolderOpen, Play, RefreshCw } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";

  let entries = $state<RecordingEntry[]>([]);
  let isLoading = $state(true);
  let thumbnails = $state<Record<string, string>>({});
  let thumbnailPass = 0;

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

  const items = $derived<RaycastListItem[]>(
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
          onAction: () => openFileLocation(entry.path),
        },
        {
          id: "copy-path",
          label: "Copy Path",
          shortcut: "⌘⇧C",
          onAction: () => copyPath(entry),
        },
      ],
    })),
  );
</script>

<RaycastList
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
</RaycastList>
