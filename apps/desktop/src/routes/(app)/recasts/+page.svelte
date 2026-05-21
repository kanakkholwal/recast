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
  import { morph } from "$lib/morph";
  import {
    Check,
    Clock,
    CopyIcon,
    ExternalLink,
    Film,
    FolderOpen,
    Grid3x3,
    List,
    ListChecks,
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
  import * as Select from "@recast/ui/select";
  import { Skeleton } from "@recast/ui/skeleton";
  import { toast } from "@recast/ui/sonner";
  import { cn } from "@recast/ui/utils";
  import { listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { SvelteSet } from "svelte/reactivity";
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

  // Multi-select: a toolbar "Select" toggle flips the page into selection
  // mode, where clicking a card checks it instead of opening the editor.
  let selectMode = $state(false);
  let bulkDeleteOpen = $state(false);
  const selected = new SvelteSet<string>();

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

  // Grid and list share one keyed {#each}. Touching `view` here gives the
  // each block a reason to re-run on a layout toggle (returning a fresh
  // array each time), which is what makes `animate:morph` fire.
  const displayed = $derived.by(() => {
    void view;
    return filtered.slice();
  });

  function activateEntry(entry: RecordingEntry) {
    if (selectMode) toggleSelected(entry.path);
    else openInEditor(entry);
  }

  function handleCardKeydown(e: KeyboardEvent, entry: RecordingEntry) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      activateEntry(entry);
    }
  }

  const selectedCount = $derived(selected.size);
  const allFilteredSelected = $derived(
    filtered.length > 0 && filtered.every((e) => selected.has(e.path)),
  );

  function exitSelectMode() {
    selectMode = false;
    selected.clear();
  }

  function toggleSelectMode() {
    if (selectMode) exitSelectMode();
    else selectMode = true;
  }

  function toggleSelected(path: string) {
    if (selected.has(path)) selected.delete(path);
    else selected.add(path);
  }

  function toggleSelectAll() {
    if (allFilteredSelected) selected.clear();
    else for (const e of filtered) selected.add(e.path);
  }

  async function handleBulkDelete() {
    const paths = [...selected];
    const results = await Promise.allSettled(
      paths.map((p) => deleteFile(p)),
    );
    const deleted = new Set<string>();
    results.forEach((r, i) => {
      if (r.status === "fulfilled") deleted.add(paths[i]);
    });
    entries = entries.filter((e) => !deleted.has(e.path));
    if (deleted.size > 0) {
      const nextThumbs = { ...thumbnails };
      for (const p of deleted) delete nextThumbs[p];
      thumbnails = nextThumbs;
    }
    const failed = paths.length - deleted.size;
    if (failed > 0) {
      toast.error(`Moved ${deleted.size} to trash · ${failed} failed`);
    } else {
      toast.success(
        `Moved ${deleted.size} recording${deleted.size === 1 ? "" : "s"} to trash`,
      );
    }
    exitSelectMode();
  }
</script>

<div class="h-full overflow-y-auto scrollbar-transparent no-scrollbar">
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
          <Button
            variant={selectMode ? "secondary" : "ghost"}
            size="xs"
            class={cn(
              "h-7 gap-1 text-[11px]",
              !selectMode && "text-muted-foreground hover:text-foreground",
            )}
            onclick={toggleSelectMode}
            disabled={entries.length === 0}
            title="Select multiple recordings"
          >
            <ListChecks size={11} />
            {selectMode ? "Done" : "Select"}
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
              aria-label="Sort recordings"
            >
              <span data-slot="select-value" class="flex items-center gap-1">
                <SortAsc size={11} />
                {sort === "recent" ? "Recent" : sort === "name" ? "Name" : "Size"}
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
                <Film class="size-3 text-muted-foreground" />
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
    {:else}
      <!-- Grid and list share one keyed {#each} so each card is the same
           DOM node in both layouts and can morph between them. -->
      <div
        class={view === "grid"
          ? "grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4"
          : "flex flex-col gap-1.5"}
      >
        {#each displayed as entry, i (entry.path)}
          {@const isSelected = selected.has(entry.path)}
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
                : "border-border/40 bg-card/40 hover:border-border hover:bg-card/70 hover:shadow-craft-sm",
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
                  <Film class={view === "grid" ? "size-6" : "size-4"} />
                </div>
              {/if}

              {#if selectMode}
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
                    <Play class="size-4 translate-x-px" />
                  </span>
                </div>
              {/if}

              {#if view === "grid"}
                <Badge
                  variant="secondary"
                  class="absolute right-1.5 top-1.5 h-4 px-1 text-[8.5px] font-bold uppercase tracking-wider backdrop-blur"
                >
                  .recast
                </Badge>
              {/if}
            </div>

            <!-- Info -->
            <div
              class={cn(
                "flex min-w-0 flex-1 flex-col gap-0.5",
                view === "grid" && "px-3 py-2.5",
              )}
            >
              <div class="truncate text-[12.5px] font-semibold text-foreground">
                {entry.filename}
              </div>
              <div class="truncate text-[10.5px] text-muted-foreground/80">
                {formatSize(entry.sizeBytes)} · {relativeDate(entry.created)}
              </div>
            </div>

            <!-- Actions -->
            {#if !selectMode}
              <div
                role="presentation"
                onclick={(e) => e.stopPropagation()}
                onkeydown={(e) => e.stopPropagation()}
                class={view === "grid" ? "absolute right-2 top-2" : "shrink-0 pr-1"}
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
                  <DropdownMenu.Content align="end" size="sm" class="w-44">
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
            {/if}
          </div>
        {/each}
      </div>
    {/if}
    </div>
  </div>
</div>

<!-- Floating bulk-action bar — visible whenever selection mode is on. -->
{#if selectMode}
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
        class="h-7 text-[11px]"
        onclick={toggleSelectAll}
        disabled={filtered.length === 0}
      >
        {allFilteredSelected ? "Clear all" : "Select all"}
      </Button>
      <Button
        variant="destructive"
        size="xs"
        class="h-7 gap-1.5 text-[11px]"
        onclick={() => (bulkDeleteOpen = true)}
        disabled={selectedCount === 0}
      >
        <Trash2 size={12} />
        Delete{selectedCount > 0 ? ` (${selectedCount})` : ""}
      </Button>
      <Button
        variant="ghost"
        size="xs"
        class="h-7 text-[11px] text-muted-foreground hover:text-foreground"
        onclick={exitSelectMode}
      >
        Cancel
      </Button>
    </div>
  </div>
{/if}

{#if bulkDeleteOpen}
  <ConfirmDialog
    open={true}
    title={`Move ${selectedCount} recording${selectedCount === 1 ? "" : "s"} to trash?`}
    description="The selected recordings will be sent to the recycle bin. You can restore them from there if needed."
    confirmLabel="Move to Trash"
    variant="destructive"
    onConfirm={handleBulkDelete}
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
