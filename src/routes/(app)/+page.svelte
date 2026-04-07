<script lang="ts">
  import { goto } from "$app/navigation";
  import { Badge } from "$components/ui/badge";
  import { Button } from "$components/ui/button";
  import {
    Card,
    CardContent,
    CardFooter,
    CardHeader,
  } from "$components/ui/card";
  import DashboardSkeleton from "$components/skeletons/DashboardSkeleton.svelte";
  import { isTauriApp } from "$lib/runtime/tauri";
  import {
    getOutputDir,
    listRecordings,
    generateThumbnails,
    openFileLocation,
    type RecordingEntry,
  } from "$lib/ipc";
  import {
    Clock3,
    ExternalLink,
    FolderOpen,
    Pencil,
    Play,
    RefreshCw,
    Video,
  } from "@lucide/svelte";
  import { onMount } from "svelte";

  type ThumbnailMap = Record<string, string>;

  let recordings = $state<RecordingEntry[]>([]);
  let isFetching = $state(true);
  let outputDir = $state("");
  let thumbnails = $state<ThumbnailMap>({});
  let thumbnailPass = 0;

  onMount(() => {
    fetchSettings();
    fetchRecordings();
  });

  async function fetchSettings() {
    try {
      outputDir = await getOutputDir();
    } catch (error) {
      console.error(error);
    }
  }

  async function fetchRecordings() {
    isFetching = true;
    thumbnails = {};
    try {
      recordings = await listRecordings();
      void loadThumbnails(recordings);
    } catch (error) {
      console.error(error);
    } finally {
      isFetching = false;
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

    const next: ThumbnailMap = {};
    for (const result of settled) {
      if (result.status === "fulfilled" && result.value[1]) {
        next[result.value[0]] = result.value[1];
      }
    }
    thumbnails = next;
  }

  async function openLocation(path: string) {
    await openFileLocation(path);
  }

  function encodeEditorPath(path: string) {
    return encodeURIComponent(btoa(encodeURIComponent(path)));
  }

  async function navigateToEditor(path: string, filename: string) {
    const route = `/editor/${encodeEditorPath(path)}`;
    const preference = localStorage.getItem("recast-editor-window");

    if (preference === "new-window" && (await isTauriApp())) {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const label = `editor-${encodeEditorPath(path)
        .replace(/[^a-zA-Z0-9]/g, "")
        .slice(0, 48)}`;
      const existing = await WebviewWindow.getByLabel(label);

      if (existing) {
        await existing.setFocus();
        return;
      }

      const editorWindow = new WebviewWindow(label, {
        url: route,
        title: `Editor - ${filename}`,
        width: 1440,
        height: 960,
        center: true,
        decorations: false,
      });
      editorWindow.once("tauri://error", (error) => console.error(error));
      return;
    }

    await goto(route);
  }

  function formatSize(bytes: number) {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  }

  function formatDate(unixSecs: number) {
    return new Date(unixSecs * 1000).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  }

  function getFileTypeLabel(filename: string) {
    const extension = filename.split(".").pop()?.toUpperCase();
    return extension || "MEDIA";
  }
</script>

<div
  class="flex w-full min-h-screen flex-1 flex-col px-6 py-8 sm:px-8 xl:px-10 overflow-hidden"
>
  <div class="mb-8 flex items-end justify-between gap-4">
    <div>
      <h2 class="text-3xl font-semibold tracking-tight text-foreground">
        Recordings
      </h2>
      <p class="mt-1.5 text-sm text-muted-foreground">
        Saved to <span
          class="rounded-md bg-muted px-1.5 py-0.5 font-mono text-xs text-foreground"
          >{outputDir || "temporary directory"}</span
        >
      </p>
    </div>

    <Button
      onclick={fetchRecordings}
      disabled={isFetching}
      class="group"
      title="Refresh"
      size="icon"
      variant="outline"
    >
      <RefreshCw
        size={16}
        class={isFetching
          ? "animate-spin"
          : "transition-transform group-hover:scale-110"}
      />
    </Button>
  </div>

  {#if isFetching}
    <DashboardSkeleton />
  {:else if recordings.length === 0}
    <div
      class="animate-in fade-in zoom-in-95 flex flex-col items-center justify-center gap-4 rounded-3xl border border-dashed border-border bg-card/50 py-32 transition-colors duration-500 hover:bg-card"
    >
      <div
        class="flex h-16 w-16 items-center justify-center rounded-2xl bg-muted text-muted-foreground shadow-[inset_0_1px_0_rgba(255,255,255,0.1)]"
      >
        <Video size={28} strokeWidth={1.5} />
      </div>
      <div class="text-center">
        <h3 class="text-base font-semibold text-foreground">
          No recordings yet
        </h3>
        <p class="mt-1.5 text-sm text-muted-foreground">
          Take your first recording from the Recast Panel.
        </p>
      </div>
    </div>
  {:else}
    <div class="grid grid-cols-1 gap-6 lg:grid-cols-2 2xl:grid-cols-3">
      {#each recordings as item, i}
        {@const thumbnail = thumbnails[item.path]}
        <Card
          class="group animate-in slide-in-from-bottom-4 fade-in relative overflow-hidden pt-0"
          style="animation-delay: {i * 45}ms;"
        >
          <CardHeader class="relative aspect-video overflow-hidden border-b p-0">
            {#if thumbnail}
              <img
                src={thumbnail}
                alt={item.filename}
                class="h-full w-full object-cover transition-transform duration-500"
                loading="lazy"
                draggable="false"
              />
            {:else}
              <div
                class="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(59,130,246,0.18),transparent_45%),linear-gradient(160deg,rgba(255,255,255,0.06),transparent_55%)]"
              ></div>
              <div class="absolute inset-0 flex items-center justify-center">
                <div
                  class="flex h-16 w-16 items-center justify-center rounded-2xl border border-white/10 bg-black/20 text-white/80 backdrop-blur"
                >
                  <Play size={28} fill="currentColor" />
                </div>
              </div>
            {/if}
          </CardHeader>

          <CardContent class="space-y-4">
            <div class="min-w-0">
              <Badge variant="secondary">
                {getFileTypeLabel(item.filename)}
              </Badge>
              <Badge variant="secondary">
                {formatSize(item.sizeBytes)}
              </Badge>
            </div>

            <h3
              class="line-clamp-1 text-base font-semibold tracking-tight text-foreground"
              title={item.filename}
            >
              {item.filename}
            </h3>
            <div
              class="mt-2 flex flex-wrap items-center gap-3 text-xs text-muted-foreground"
            >
              <span class="inline-flex items-center gap-1.5">
                <Clock3 size={13} />
                {formatDate(item.created)}
              </span>
              <span class="inline-flex items-center gap-1.5">
                <FolderOpen size={13} />
                Recording
              </span>
            </div>
          </CardContent>
          <CardFooter class="gap-3">
            <Button
              type="button"
              onclick={() => navigateToEditor(item.path, item.filename)}
              size="icon"
              class="rounded-full shadow shadow-primary/40"
            >
              <Pencil size={16} />
            </Button>
            <div>
              <p class="text-sm font-semibold text-foreground">Open in editor</p>
              <p class="text-xs text-muted-foreground">
                Continue trimming and exporting
              </p>
            </div>
            <Button
              type="button"
              onclick={() => openLocation(item.path)}
              size="icon"
              variant="secondary"
              class="ml-auto rounded-full"
              title="Show in folder"
            >
              <ExternalLink size={15} />
            </Button>
          </CardFooter>
        </Card>
      {/each}
    </div>
  {/if}
</div>
