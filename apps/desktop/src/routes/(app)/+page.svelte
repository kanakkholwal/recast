<script lang="ts">
  import { goto } from "$app/navigation";
  import { commandPalette } from "$lib/stores/command-palette.svelte";
  import {
    generateThumbnails,
    getOutputDir,
    launchRecordingPanel,
    listExports,
    listRecasts,
    openFileLocation,
    type RecordingEntry,
  } from "$lib/ipc";
  import {
    AppWindow,
    ArrowRight,
    Camera,
    Crop,
    Download,
    Film,
    FolderOpen,
    Mic,
    Monitor,
    Radio,
    Search,
    Sparkles,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { Kbd } from "@recast/ui/kbd";
  import { toast } from "@recast/ui/sonner";
  import { cn } from "@recast/ui/utils";
  import { listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  let recasts = $state<RecordingEntry[]>([]);
  let exports_ = $state<RecordingEntry[]>([]);
  let isLoading = $state(true);
  let thumbnails = $state<Record<string, string>>({});
  let editorWindow = $state<"navigate" | "new-window">("navigate");
  let now = $state(Date.now());
  let thumbnailPass = 0;

  onMount(() => {
    fetchAll();
    const stored = localStorage.getItem("recast-editor-window") as
      | "navigate"
      | "new-window"
      | null;
    if (stored) editorWindow = stored;
    const unlisten = listen("refresh-recordings", () => fetchAll());
    const tick = window.setInterval(() => (now = Date.now()), 60_000);
    return () => {
      unlisten.then((fn) => fn());
      window.clearInterval(tick);
    };
  });

  async function fetchAll() {
    isLoading = true;
    try {
      const [r, e] = await Promise.all([listRecasts(), listExports()]);
      recasts = r.sort((a, b) => b.created - a.created).slice(0, 6);
      exports_ = e.sort((a, b) => b.created - a.created).slice(0, 6);
      void loadThumbnails([...recasts, ...exports_]);
    } catch (err) {
      toast.error(`Could not load activity: ${err}`);
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

  const greeting = $derived.by(() => {
    const h = new Date(now).getHours();
    if (h < 5) return "Working late";
    if (h < 12) return "Good morning";
    if (h < 17) return "Good afternoon";
    if (h < 21) return "Good evening";
    return "Good night";
  });

  function formatSize(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  }

  function formatDate(unix: number) {
    const diff = now / 1000 - unix;
    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    if (diff < 86400 * 7) return `${Math.floor(diff / 86400)}d ago`;
    return new Date(unix * 1000).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  }

  function encodeEditorPath(path: string) {
    return encodeURIComponent(btoa(encodeURIComponent(path)));
  }

  async function openInEditor(entry: RecordingEntry) {
    const route = `/editor/${encodeEditorPath(entry.path)}`;
    if (editorWindow === "new-window") {
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
    } else {
      goto(route);
    }
  }

  async function showOutputFolder() {
    try {
      const dir = await getOutputDir();
      await openFileLocation(dir);
    } catch (err) {
      toast.error(`Could not open folder: ${err}`);
    }
  }

  async function openDevicePickerWindow(type: "mic" | "camera") {
    const label = `device-picker-${type}`;
    const existing = await WebviewWindow.getByLabel(label);
    if (existing) {
      await existing.setFocus();
      return;
    }
    new WebviewWindow(label, {
      url: `/device-picker?type=${type}`,
      title: `Select ${type === "mic" ? "Microphone" : "Camera"}`,
      width: 320,
      height: 340,
      center: true,
      decorations: false,
      transparent: true,
      shadow: false,
      resizable: false,
    });
  }

  async function openCameraPreviewWindow() {
    const existing = await WebviewWindow.getByLabel("camera-preview");
    if (existing) {
      await existing.setFocus();
      return;
    }
    new WebviewWindow("camera-preview", {
      url: "/camera-preview",
      title: "Camera",
      width: 320,
      height: 320,
      decorations: false,
      transparent: true,
      shadow: false,
      alwaysOnTop: true,
      resizable: true,
      center: true,
    });
  }

  // Recording modes — each launches the panel; the panel honors the last
  // chosen source. The mode hint is purely visual prompting today.
  const modes = [
    {
      id: "screen",
      label: "Full Screen",
      hint: "Capture an entire display",
      icon: Monitor,
    },
    {
      id: "window",
      label: "Window",
      hint: "Capture a single app window",
      icon: AppWindow,
    },
    {
      id: "region",
      label: "Region",
      hint: "Drag to select an area",
      icon: Crop,
    },
    {
      id: "camera",
      label: "Camera",
      hint: "Webcam-only capture",
      icon: Camera,
    },
  ] as const;

  // Quick actions surfaced as chips below the modes.
  type QuickAction = {
    id: string;
    label: string;
    icon: typeof Mic;
    onClick: () => void;
  };
  const quickActions: QuickAction[] = [
    {
      id: "preview",
      label: "Camera preview",
      icon: Camera,
      onClick: () => openCameraPreviewWindow(),
    },
    {
      id: "mic",
      label: "Pick microphone",
      icon: Mic,
      onClick: () => openDevicePickerWindow("mic"),
    },
    {
      id: "cam",
      label: "Pick camera",
      icon: Camera,
      onClick: () => openDevicePickerWindow("camera"),
    },
    {
      id: "folder",
      label: "Show folder",
      icon: FolderOpen,
      onClick: () => showOutputFolder(),
    },
  ];
</script>

<div class="h-full overflow-y-auto scrollbar-transparent">
  <div class="mx-auto flex max-w-3xl flex-col gap-10 px-6 py-12 md:py-16">
    <!-- Hero -->
    <header
      in:fly={{ y: 12, duration: 360, easing: cubicOut }}
      class="flex flex-col items-center gap-3 text-center"
    >
      <span
        class="inline-flex items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.15em] text-muted-foreground/80 backdrop-blur"
      >
        <Sparkles class="size-3 text-primary" />
        Recast
      </span>
      <h1
        class="text-balance text-[34px] font-semibold leading-tight tracking-tight text-foreground md:text-[40px]"
      >
        {greeting}.
        <span class="bg-gradient-to-r from-foreground to-foreground/55 bg-clip-text text-transparent">
          What do you want to capture?
        </span>
      </h1>
      <p class="max-w-md text-[13px] leading-relaxed text-muted-foreground">
        Pick a mode below or jump into the panel. Press
        <Kbd class="mx-0.5 align-middle">
          <span class="text-[8px] font-semibold">⌘</span>
          <span class="text-[11px]">K</span>
        </Kbd>
        anywhere to search every action.
      </p>
    </header>

    <!-- Search bar (opens command palette) -->
    <button
      type="button"
      onclick={() => commandPalette.show()}
      in:fly={{ y: 12, duration: 360, delay: 60, easing: cubicOut }}
      class="group/search flex h-12 items-center gap-3 rounded-xl border border-border/60 bg-card/70 px-4 text-left shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:border-border hover:bg-card hover:shadow-craft-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      <Search
        class="size-4 shrink-0 text-muted-foreground/70 transition-colors group-hover/search:text-foreground"
      />
      <span class="flex-1 text-[13px] font-medium text-muted-foreground/80">
        Search actions, recordings, exports…
      </span>
      <Kbd>
        <span class="text-[8px] font-semibold">⌘</span>
        <span class="text-[11px]">K</span>
      </Kbd>
    </button>

    <!-- Recording modes -->
    <section
      in:fly={{ y: 12, duration: 360, delay: 120, easing: cubicOut }}
      class="flex flex-col gap-3"
    >
      <div class="flex items-baseline justify-between px-1">
        <h2 class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70">
          Start a recording
        </h2>
        <Button
          variant="ghost"
          size="xs"
          class="h-7 gap-1 text-[11px] text-muted-foreground hover:text-foreground"
          onclick={launchRecordingPanel}
        >
          Open panel
          <Kbd class="hidden sm:inline-flex">⌘⇧R</Kbd>
        </Button>
      </div>
      <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {#each modes as mode, i (mode.id)}
          {@const Icon = mode.icon}
          <button
            type="button"
            onclick={launchRecordingPanel}
            in:fly={{
              y: 8,
              duration: 320,
              delay: 160 + i * 40,
              easing: cubicOut,
            }}
            class={cn(
              "group/mode relative flex aspect-[5/4] flex-col items-start justify-between overflow-hidden rounded-xl border border-border/60 bg-card/70 p-3 text-left shadow-(--shadow-craft-inset) backdrop-blur",
              "transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-craft-sm",
              "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
            )}
          >
            <span
              class="flex size-8 items-center justify-center rounded-lg bg-foreground/5 text-foreground transition-colors group-hover/mode:bg-primary/10 group-hover/mode:text-primary"
            >
              <Icon class="size-4" />
            </span>
            <div class="flex w-full items-end justify-between gap-2">
              <div class="min-w-0">
                <div class="truncate text-[12.5px] font-semibold text-foreground">
                  {mode.label}
                </div>
                <div class="truncate text-[10.5px] text-muted-foreground/80">
                  {mode.hint}
                </div>
              </div>
              <ArrowRight
                class="size-3.5 shrink-0 text-muted-foreground/50 transition-all duration-200 group-hover/mode:translate-x-0.5 group-hover/mode:text-foreground"
              />
            </div>
          </button>
        {/each}
      </div>
    </section>

    <!-- Primary CTA + quick action chips -->
    <section
      in:fly={{ y: 12, duration: 360, delay: 200, easing: cubicOut }}
      class="flex flex-col gap-3"
    >
      <Button
        onclick={launchRecordingPanel}
        size="lg"
        class="group/cta h-12 w-full gap-2 rounded-xl text-[13px] font-semibold"
      >
        <Radio class="size-4 transition-transform duration-200 group-hover/cta:rotate-12" />
        Launch recording panel
        <Kbd class="ml-1 bg-primary-foreground/15 text-primary-foreground/90">
          ⌘⇧R
        </Kbd>
      </Button>
      <div class="flex flex-wrap gap-2">
        {#each quickActions as qa (qa.id)}
          {@const Icon = qa.icon}
          <button
            type="button"
            onclick={qa.onClick}
            class="inline-flex h-8 items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-3 text-[11.5px] font-medium text-muted-foreground transition-all duration-200 hover:-translate-y-px hover:border-border hover:bg-card hover:text-foreground hover:shadow-craft-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            <Icon class="size-3.5" />
            {qa.label}
          </button>
        {/each}
      </div>
    </section>

    <!-- Recent strips -->
    {#if recasts.length > 0 || isLoading}
      <section class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between px-1">
          <h2 class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70">
            Recent recordings
          </h2>
          <Button
            variant="ghost"
            size="xs"
            class="h-7 gap-1 text-[11px] text-muted-foreground hover:text-foreground"
            onclick={() => goto("/recasts")}
          >
            See all
            <ArrowRight class="size-3" />
          </Button>
        </div>
        <div
          class="-mx-1 flex gap-2 overflow-x-auto px-1 pb-1 scrollbar-transparent"
        >
          {#if isLoading && recasts.length === 0}
            {#each Array.from({ length: 4 }) as _, i (i)}
              <div
                class="aspect-video w-44 shrink-0 animate-pulse rounded-lg bg-muted/60"
                style="animation-delay: {i * 100}ms"
              ></div>
            {/each}
          {:else}
            {#each recasts as entry, i (entry.path)}
              <button
                type="button"
                onclick={() => openInEditor(entry)}
                in:fade={{ duration: 220, delay: i * 40 }}
                class="group/card flex w-44 shrink-0 flex-col gap-1.5 rounded-lg p-1 text-left transition-all duration-200 hover:bg-card/60 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              >
                <div
                  class="relative aspect-video overflow-hidden rounded-md border border-border/40 bg-muted/40 shadow-(--shadow-craft-inset) transition-transform duration-200 group-hover/card:-translate-y-0.5 group-hover/card:shadow-craft-sm"
                >
                  {#if thumbnails[entry.path]}
                    <img
                      src={thumbnails[entry.path]}
                      alt=""
                      class="h-full w-full object-cover"
                    />
                  {:else}
                    <div class="grid h-full w-full place-items-center text-muted-foreground/50">
                      <Film class="size-5" />
                    </div>
                  {/if}
                </div>
                <div class="px-1">
                  <div class="truncate text-[11.5px] font-medium text-foreground">
                    {entry.filename}
                  </div>
                  <div class="truncate text-[10px] text-muted-foreground/70">
                    {formatSize(entry.sizeBytes)} · {formatDate(entry.created)}
                  </div>
                </div>
              </button>
            {/each}
          {/if}
        </div>
      </section>
    {/if}

    {#if exports_.length > 0}
      <section class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between px-1">
          <h2 class="text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70">
            Recent exports
          </h2>
          <Button
            variant="ghost"
            size="xs"
            class="h-7 gap-1 text-[11px] text-muted-foreground hover:text-foreground"
            onclick={() => goto("/exports")}
          >
            See all
            <ArrowRight class="size-3" />
          </Button>
        </div>
        <div
          class="-mx-1 flex gap-2 overflow-x-auto px-1 pb-1 scrollbar-transparent"
        >
          {#each exports_ as entry, i (entry.path)}
            <button
              type="button"
              onclick={() => openFileLocation(entry.path)}
              in:fade={{ duration: 220, delay: i * 40 }}
              class="group/card flex w-44 shrink-0 flex-col gap-1.5 rounded-lg p-1 text-left transition-all duration-200 hover:bg-card/60 focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <div
                class="relative aspect-video overflow-hidden rounded-md border border-border/40 bg-muted/40 shadow-(--shadow-craft-inset) transition-transform duration-200 group-hover/card:-translate-y-0.5 group-hover/card:shadow-craft-sm"
              >
                {#if thumbnails[entry.path]}
                  <img
                    src={thumbnails[entry.path]}
                    alt=""
                    class="h-full w-full object-cover"
                  />
                {:else}
                  <div class="grid h-full w-full place-items-center text-muted-foreground/50">
                    <Download class="size-5" />
                  </div>
                {/if}
                <span
                  class="absolute right-1 top-1 rounded-sm bg-background/85 px-1 py-px text-[8.5px] font-bold uppercase tracking-wider text-foreground/80 backdrop-blur"
                >
                  {entry.filename.split(".").pop() ?? ""}
                </span>
              </div>
              <div class="px-1">
                <div class="truncate text-[11.5px] font-medium text-foreground">
                  {entry.filename}
                </div>
                <div class="truncate text-[10px] text-muted-foreground/70">
                  {formatSize(entry.sizeBytes)} · {formatDate(entry.created)}
                </div>
              </div>
            </button>
          {/each}
        </div>
      </section>
    {/if}

    {#if !isLoading && recasts.length === 0 && exports_.length === 0}
      <div
        class="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border/60 bg-card/40 p-8 text-center"
      >
        <div
          class="flex size-10 items-center justify-center rounded-xl bg-foreground/5 text-foreground"
        >
          <Film class="size-5" />
        </div>
        <div>
          <p class="text-[13px] font-semibold text-foreground">
            No recordings yet
          </p>
          <p class="mt-1 text-[11px] text-muted-foreground">
            Launch the panel above to capture your first clip.
          </p>
        </div>
      </div>
    {/if}
  </div>
</div>
