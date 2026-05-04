<script lang="ts">
  import { goto } from "$app/navigation";
  import { RecastList, type RecastListItem } from "$components/recast";
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
    Camera,
    CopyIcon,
    Download,
    Film,
    FolderOpen,
    Mic,
    Monitor,
    Radio,
    Settings as SettingsIcon,
    SlidersHorizontal,

    VideotapeIcon
  } from "@lucide/svelte";
  import { toast } from "@recast/ui/sonner";
  import { listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";

  let recasts = $state<RecordingEntry[]>([]);
  let exports_ = $state<RecordingEntry[]>([]);
  let isLoading = $state(true);
  let thumbnails = $state<Record<string, string>>({});
  let editorWindow = $state<"navigate" | "new-window">("navigate");
  let thumbnailPass = 0;

  onMount(() => {
    fetchAll();
    const stored = localStorage.getItem("recast-editor-window") as "navigate" | "new-window" | null;
    if (stored) editorWindow = stored;
    const unlisten = listen("refresh-recordings", () => fetchAll());
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  async function fetchAll() {
    isLoading = true;
    try {
      const [r, e] = await Promise.all([listRecasts(), listExports()]);
      recasts = r.sort((a, b) => b.created - a.created).slice(0, 5);
      exports_ = e.sort((a, b) => b.created - a.created).slice(0, 5);
      loadThumbnails([...recasts, ...exports_]);
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

  function formatSize(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  }

  function formatDate(unix: number) {
    const now = Date.now() / 1000;
    const diff = now - unix;
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
      const label = `editor-${encodeEditorPath(entry.path).replace(/[^a-zA-Z0-9]/g, "").slice(0, 48)}`;
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

  async function copyPath(entry: RecordingEntry) {
    try {
      await navigator.clipboard.writeText(entry.path);
      toast.success("Path copied");
    } catch (err) {
      toast.error(`Copy failed: ${err}`);
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

  /** Open (or focus) the device-picker window for mic/camera. Mirrors the panel's logic. */
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
      resizable: false,
    });
  }

  /** Open (or focus) the floating camera-preview window (always-on-top, transparent). */
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

  const items = $derived<RecastListItem[]>([
    // Quick actions
    {
      id: "action-launch",
      title: "Launch Recording Panel",
      subtitle: "Start a new screen recording",
      icon: Radio,
      iconClass: "text-primary",
      keywords: ["record", "start", "capture", "panel"],
      section: "Quick Actions",
      layout: "row",
      onSelect: () => launchRecordingPanel(),
      actions: [
        {
          id: "launch",
          label: "Launch Panel",
          icon: Radio,
          shortcut: "⌘⇧R",
          onAction: () => launchRecordingPanel(),
        },
      ],
    },
    {
      id: "action-pick-camera",
      title: "Pick Camera",
      subtitle: "Choose a webcam device",
      icon: Monitor,
      keywords: ["camera", "webcam", "device", "source", "picker", "video"],
      section: "Quick Actions",
      layout: "row",
      onSelect: () => openDevicePickerWindow("camera"),
      actions: [
        {
          id: "pick-camera",
          label: "Open Camera Picker",
          icon: Camera,
          onAction: () => openDevicePickerWindow("camera"),
        },
        {
          id: "pick-mic",
          label: "Open Microphone Picker",
          icon: Mic,
          onAction: () => openDevicePickerWindow("mic"),
        },
      ],
    },
    {
      id: "action-pick-microphone",
      title: "Pick Microphone",
      subtitle: "Choose an audio input device",
      icon: Mic,
      keywords: ["microphone", "mic", "audio", "device", "input", "picker"],
      section: "Quick Actions",
      layout: "row",
      onSelect: () => openDevicePickerWindow("mic"),
      actions: [
        {
          id: "pick-mic",
          label: "Open Microphone Picker",
          icon: Mic,
          onAction: () => openDevicePickerWindow("mic"),
        },
        {
          id: "pick-camera",
          label: "Open Camera Picker",
          icon: Camera,
          onAction: () => openDevicePickerWindow("camera"),
        },
      ],
    },
    {
      id: "action-camera-preview",
      title: "Camera Preview",
      subtitle: "Test your webcam feed in a floating window",
      icon: Camera,
      keywords: ["camera", "webcam", "preview", "test", "floating"],
      section: "Quick Actions",
      layout: "row",
      onSelect: () => openCameraPreviewWindow(),
      actions: [
        {
          id: "preview",
          label: "Open Camera Preview",
          icon: Camera,
          onAction: () => openCameraPreviewWindow(),
        },
      ],
    },

    // Navigation
    {
      id: "nav-recasts",
      title: "All Recordings",
      subtitle: `${recasts.length > 0 ? `${recasts.length}+ recent` : "Browse your recordings"}`,
      icon: Film,
      keywords: ["recordings", "recasts", "library", "all"],
      section: "Browse",
      layout: "row",
      onSelect: () => goto("/recasts"),
      actions: [
        { id: "open", label: "Go to Recordings", icon: Film, onAction: () => goto("/recasts") },
      ],
    },
    {
      id: "nav-exports",
      title: "All Exports",
      subtitle: "Browse exported videos ready to share",
      icon: Download,
      keywords: ["exports", "rendered", "share"],
      section: "Browse",
      layout: "row",
      onSelect: () => goto("/exports"),
      actions: [
        { id: "open", label: "Go to Exports", icon: Download, onAction: () => goto("/exports") },
      ],
    },
    {
      id: "nav-profiles",
      title: "Recording Profiles",
      subtitle: "Manage your recording presets",
      icon: SlidersHorizontal,
      keywords: ["profiles", "presets", "config"],
      section: "Browse",
      layout: "row",
      onSelect: () => goto("/profiles"),
      actions: [
        {
          id: "open",
          label: "Go to Profiles",
          icon: SlidersHorizontal,
          onAction: () => goto("/profiles"),
        },
      ],
    },
    {
      id: "nav-settings",
      title: "Settings",
      subtitle: "Configure Recast preferences",
      icon: SettingsIcon,
      keywords: ["settings", "preferences", "config"],
      section: "Browse",
      layout: "row",
      onSelect: () => goto("/settings"),
      actions: [
        {
          id: "open",
          label: "Go to Settings",
          icon: SettingsIcon,
          onAction: () => goto("/settings"),
        },
      ],
    },
    {
      id: "action-show-folder",
      title: "Show Output Folder",
      subtitle: "Reveal the recordings directory in Explorer",
      icon: FolderOpen,
      keywords: ["folder", "directory", "reveal", "explorer", "finder"],
      section: "Browse",
      layout: "row",
      onSelect: () => showOutputFolder(),
      actions: [
        {
          id: "show",
          label: "Show in Folder",
          icon: FolderOpen,
          onAction: () => showOutputFolder(),
        },
      ],
    },

    // Recent recordings
    ...recasts.map<RecastListItem>((entry) => ({
      id: `recast-${entry.path}`,
      title: entry.filename,
      subtitle: `${formatSize(entry.sizeBytes)} · ${formatDate(entry.created)}`,
      icon: Film,
      iconImage: thumbnails[entry.path],
      keywords: [entry.filename, "recording", "recent"],
      accessories: [{ text: ".recast", variant: "info" }],
      section: "Recent Recordings",
      onSelect: () => openInEditor(entry),
      actions: [
        { id: "open", label: "Open in Editor", icon: VideotapeIcon, onAction: () => openInEditor(entry) },
        {
          id: "show",
          label: "Show in Folder",
          icon: FolderOpen,
          shortcut: "⌘O",
          onAction: () => openFileLocation(entry.path),
        },
        {
          id: "copy",
          label: "Copy Path",
          shortcut: "⌘⇧C",
          icon: CopyIcon,
          onAction: () => copyPath(entry),
        },
      ],
    })),

    // Recent exports
    ...exports_.map<RecastListItem>((entry) => ({
      id: `export-${entry.path}`,
      title: entry.filename,
      subtitle: `${formatSize(entry.sizeBytes)} · ${formatDate(entry.created)}`,
      icon: Download,
      iconImage: thumbnails[entry.path],
      keywords: [entry.filename, "export", "recent"],
      accessories: [
        {
          text: entry.filename.split(".").pop()?.toUpperCase() ?? "FILE",
          variant: "default",
        },
      ],
      section: "Recent Exports",
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
          id: "copy",
          label: "Copy Path",
          shortcut: "⌘⇧C",
          icon: CopyIcon,
          onAction: () => copyPath(entry),
        },
      ],
    })),
  ]);
</script>

<RecastList
  {items}
  {isLoading}
  title="Home"
  subtitle="Type to search across actions, recordings and exports"
  searchPlaceholder="What do you want to do?"
  emptyTitle="No matches"
  emptyHint="Try a different term — or press ⌘ + K to see everything."
/>
