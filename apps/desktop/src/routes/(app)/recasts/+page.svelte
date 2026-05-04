<script lang="ts">
  import { goto } from "$app/navigation";
  import { ConfirmDialog, RenameDialog } from "$components/recast";
  import {
    deleteFile,
    generateThumbnails,
    listRecasts,
    openFileLocation,
    renameFile,
    type RecordingEntry,
  } from "$lib/ipc";
  import {
    Clock,
    CopyIcon,
    ExternalLink,
    Film,
    FolderOpen,
    Grid3x3,
    List,
    MoreHorizontal,
    Pencil,
    Play,
    RefreshCw,
    Search,
    SortAsc,
    Trash2,
    X,
  } from "@lucide/svelte";
  import { Badge } from "@recast/ui/badge";
  import { Button } from "@recast/ui/button";
  import { ButtonGroup } from "@recast/ui/button-group";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
  import { Kbd } from "@recast/ui/kbd";
  import { Skeleton } from "@recast/ui/skeleton";
  import { toast } from "@recast/ui/sonner";
  import { cn } from "@recast/ui/utils";
  import { listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  let entries = $state<RecordingEntry[]>([]);
  let isLoading = $state(true);
  let thumbnails = $state<Record<string, string>>({});
  let editorWindow = $state<"navigate" | "new-window">("navigate");
  let thumbnailPass = 0;

  let query = $state("");
  let view = $state<"grid" | "list">("grid");
  let sort = $state<"recent" | "name" | "size">("recent");
  let renameTarget = $state<RecordingEntry | null>(null);
  let deleteTarget = $state<RecordingEntry | null>(null);

  onMount(() => {
    fetchRecasts();
    const stored = localStorage.getItem("recast-editor-window") as
      | "navigate"
      | "new-window"
      | null;
    if (stored) editorWindow = stored;
    const storedView = localStorage.getItem("recasts-view") as
      | "grid"
      | "list"
      | null;
    if (storedView) view = storedView;
    const unlisten = listen("refresh-recordings", () => fetchRecasts());
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  $effect(() => {
    localStorage.setItem("recasts-view", view);
  });

  async function fetchRecasts() {
    isLoading = true;
    try {
      entries = await listRecasts();
      void loadThumbnails(entries);
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

  function relativeDate(unix: number) {
    const diff = Date.now() / 1000 - unix;
    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    if (diff < 86400 * 7) return `${Math.floor(diff / 86400)}d ago`;
    return formatDate(unix);
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

  async function copyPath(entry: RecordingEntry) {
    try {
      await navigator.clipboard.writeText(entry.path);
      toast.success("Path copied");
    } catch (e) {
      toast.error(`Copy failed: ${e}`);
    }
  }

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    let list = q
      ? entries.filter((e) => e.filename.toLowerCase().includes(q))
      : entries.slice();
    if (sort === "recent") list.sort((a, b) => b.created - a.created);
    else if (sort === "name")
      list.sort((a, b) => a.filename.localeCompare(b.filename));
    else if (sort === "size") list.sort((a, b) => b.sizeBytes - a.sizeBytes);
    return list;
  });

  const totalSize = $derived(
    entries.reduce((sum, e) => sum + e.sizeBytes, 0),
  );
</script>

<div class="h-full overflow-y-auto scrollbar-transparent">
  <div class="mx-auto flex max-w-5xl flex-col gap-8 px-6 py-10">
    <!-- Hero (mirrors home page rhythm: eyebrow + heading + helper) -->
    <header
      in:fly={{ y: 12, duration: 320, easing: cubicOut }}
      class="flex flex-col gap-3"
    >
      <span
        class="inline-flex w-fit items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.15em] text-muted-foreground/80 backdrop-blur"
      >
        <Film class="size-3 text-primary" />
        Library
      </span>
      <h1
        class="text-balance text-[28px] font-semibold leading-tight tracking-tight text-foreground md:text-[32px]"
      >
        <span
          class="bg-linear-to-r from-foreground to-foreground/55 bg-clip-text text-transparent"
        >
          {entries.length === 0
            ? "No recordings yet"
            : entries.length === 1
              ? "1 recording"
              : `${entries.length} recordings`}
        </span>
      </h1>
      <p class="text-[12.5px] leading-relaxed text-muted-foreground">
        {formatSize(totalSize)} on disk · open any clip in the editor or use ⌘K to jump anywhere.
      </p>
    </header>

    <!-- Hero search bar (matches home: h-12 pill, inset shadow, hover lift) -->
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
        placeholder="Search recordings…"
        aria-label="Search recordings"
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

    <!-- Section header: matches home page sections (uppercase eyebrow + actions on the right) -->
    <div
      in:fly={{ y: 12, duration: 320, delay: 120, easing: cubicOut }}
      class="flex flex-col gap-3"
    >
      <div class="flex items-center justify-between gap-3 px-1">
        <h2
          class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
        >
          {query ? `Results for “${query}”` : "All recordings"}
        </h2>
        <div class="flex items-center gap-1.5">
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props as Record<string, unknown>}
                  variant="ghost"
                  size="xs"
                  class="h-7 gap-1 text-[11px] text-muted-foreground hover:text-foreground"
                >
                  <SortAsc size={11} />
                  {sort === "recent"
                    ? "Recent"
                    : sort === "name"
                      ? "Name"
                      : "Size"}
                </Button>
              {/snippet}
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="end" size="sm" class="w-36">
              <DropdownMenu.Item onSelect={() => (sort = "recent")}>
                <Clock class="text-muted-foreground" /> Recent
              </DropdownMenu.Item>
              <DropdownMenu.Item onSelect={() => (sort = "name")}>
                <SortAsc class="text-muted-foreground" /> Name
              </DropdownMenu.Item>
              <DropdownMenu.Item onSelect={() => (sort = "size")}>
                <Film class="text-muted-foreground" /> Size
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Root>

          <ButtonGroup>
            <Button
              variant={view === "grid" ? "secondary" : "outline"}
              size="icon-sm"
              onclick={() => (view = "grid")}
              title="Grid view"
            >
              <Grid3x3 size={12} />
            </Button>
            <Button
              variant={view === "list" ? "secondary" : "outline"}
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
            onclick={fetchRecasts}
            disabled={isLoading}
            title="Refresh"
          >
            <RefreshCw
              size={12}
              class={isLoading ? "animate-spin" : ""}
            />
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
          <Film class="size-5" />
        </div>
        <div>
          <p class="text-[14px] font-semibold text-foreground">
            {query ? "No matches" : "No recordings yet"}
          </p>
          <p class="mt-1 text-[11.5px] text-muted-foreground">
            {query
              ? `Nothing matches "${query}".`
              : "Capture your first clip from the recording panel."}
          </p>
        </div>
      </div>
    {:else if view === "grid"}
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        {#each filtered as entry, i (entry.path)}
          <div
            in:fade={{ duration: 200, delay: Math.min(i * 30, 240) }}
            class="group/card relative flex flex-col gap-2"
          >
            <button
              type="button"
              onclick={() => openInEditor(entry)}
              class="relative aspect-video overflow-hidden rounded-xl border border-border/40 bg-muted/40 shadow-(--shadow-craft-inset) transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-craft-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
              title={entry.filename}
            >
              {#if thumbnails[entry.path]}
                <img
                  src={thumbnails[entry.path]}
                  alt=""
                  class="h-full w-full object-cover transition-transform duration-300 group-hover/card:scale-[1.03]"
                />
              {:else}
                <div
                  class="grid h-full w-full place-items-center text-muted-foreground/50"
                >
                  <Film class="size-6" />
                </div>
              {/if}
              <div
                class="pointer-events-none absolute inset-0 bg-linear-to-t from-black/40 via-transparent to-transparent opacity-0 transition-opacity duration-200 group-hover/card:opacity-100"
              ></div>
              <div
                class="pointer-events-none absolute inset-0 grid place-items-center opacity-0 transition-opacity duration-200 group-hover/card:opacity-100"
              >
                <span
                  class="flex size-9 items-center justify-center rounded-full bg-background/85 text-foreground shadow-craft-sm backdrop-blur"
                >
                  <Play class="size-4 translate-x-px" />
                </span>
              </div>
              <Badge
                variant="secondary"
                class="absolute right-1.5 top-1.5 h-4 px-1 text-[8.5px] font-bold uppercase tracking-wider backdrop-blur"
              >
                .recast
              </Badge>
            </button>

            <div class="flex items-start justify-between gap-2 px-1">
              <div class="min-w-0 flex-1">
                <div class="truncate text-[12px] font-semibold text-foreground">
                  {entry.filename}
                </div>
                <div class="truncate text-[10.5px] text-muted-foreground/80">
                  {formatSize(entry.sizeBytes)} · {relativeDate(entry.created)}
                </div>
              </div>
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button
                      {...props as Record<string, unknown>}
                      variant="ghost"
                      size="icon-sm"
                      class="-mr-1 size-6 opacity-0 transition-opacity duration-200 group-hover/card:opacity-100 focus-visible:opacity-100"
                      title="More actions"
                    >
                      <MoreHorizontal size={13} />
                    </Button>
                  {/snippet}
                </DropdownMenu.Trigger>
                <DropdownMenu.Content
                  align="end"
                  size="sm"
                  class="w-44"
                >
                  <DropdownMenu.Item onSelect={() => openInEditor(entry)}>
                    <Pencil class="size-3" /> Open in editor
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => openInNewWindow(entry)}>
                    <ExternalLink class="size-3" /> New window
                    <DropdownMenu.Shortcut>
                      <Kbd>⌘↵</Kbd>
                    </DropdownMenu.Shortcut>
                  </DropdownMenu.Item>
                  <DropdownMenu.Separator />
                  <DropdownMenu.Item onSelect={() => (renameTarget = entry)}>
                    <Pencil class="size-3" /> Rename…
                    <DropdownMenu.Shortcut>
                      <Kbd>⌘R</Kbd>
                    </DropdownMenu.Shortcut>
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    onSelect={() => openFileLocation(entry.path)}
                  >
                    <FolderOpen class="size-3" /> Show in folder
                    <DropdownMenu.Shortcut>
                      <Kbd>⌘O</Kbd>
                    </DropdownMenu.Shortcut>
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => copyPath(entry)}>
                    <CopyIcon class="size-3" /> Copy path
                  </DropdownMenu.Item>
                  <DropdownMenu.Separator />
                  <DropdownMenu.Item
                    onSelect={() => (deleteTarget = entry)}
                    class="text-destructive focus:bg-destructive/10 focus:text-destructive"
                  >
                    <Trash2 class="size-3" /> Move to trash
                  </DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <!-- List view: each row is a Card with leading thumbnail + metadata + dropdown -->
      <div class="flex flex-col gap-1">
        {#each filtered as entry, i (entry.path)}
          <div
            in:fade={{ duration: 180, delay: Math.min(i * 20, 200) }}
            class="group/row flex items-center gap-3 rounded-lg border border-transparent px-2 py-1.5 transition-colors hover:border-border/40 hover:bg-card/60"
          >
            <button
              type="button"
              onclick={() => openInEditor(entry)}
              class="flex flex-1 items-center gap-3 rounded-lg text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
            >
              <div
                class="relative aspect-video w-20 shrink-0 overflow-hidden rounded-md border border-border/40 bg-muted/40 shadow-(--shadow-craft-inset)"
              >
                {#if thumbnails[entry.path]}
                  <img
                    src={thumbnails[entry.path]}
                    alt=""
                    class="h-full w-full object-cover"
                  />
                {:else}
                  <div
                    class="grid h-full w-full place-items-center text-muted-foreground/50"
                  >
                    <Film class="size-4" />
                  </div>
                {/if}
              </div>
              <div class="min-w-0 flex-1">
                <div class="truncate text-[12.5px] font-semibold text-foreground">
                  {entry.filename}
                </div>
                <div class="truncate text-[10.5px] text-muted-foreground/80">
                  {formatSize(entry.sizeBytes)} · {formatDate(entry.created)}
                </div>
              </div>
            </button>

            <DropdownMenu.Root>
              <DropdownMenu.Trigger>
                {#snippet child({ props })}
                  <Button
                    {...props as Record<string, unknown>}
                    variant="ghost"
                    size="icon-sm"
                    class="size-7 opacity-0 transition-opacity duration-150 group-hover/row:opacity-100 focus-visible:opacity-100"
                    title="More actions"
                  >
                    <MoreHorizontal size={14} />
                  </Button>
                {/snippet}
              </DropdownMenu.Trigger>
              <DropdownMenu.Content align="end" size="sm" class="w-44">
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
                <DropdownMenu.Item
                  onSelect={() => openFileLocation(entry.path)}
                >
                  <FolderOpen class="size-3" /> Show in folder
                </DropdownMenu.Item>
                <DropdownMenu.Item onSelect={() => copyPath(entry)}>
                  <CopyIcon class="size-3" /> Copy path
                </DropdownMenu.Item>
                <DropdownMenu.Separator />
                <DropdownMenu.Item
                  onSelect={() => (deleteTarget = entry)}
                  class="text-destructive focus:bg-destructive/10 focus:text-destructive"
                >
                  <Trash2 class="size-3" /> Move to trash
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Root>
          </div>
        {/each}
      </div>
    {/if}
    </div>
  </div>
</div>

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
