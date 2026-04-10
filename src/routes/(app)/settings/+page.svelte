<script lang="ts">
  import { Button } from "$components/ui/button";
  import { config } from "$constants/app";
  import { getOutputDir, setOutputDir } from "$lib/ipc";
  import {
    ArrowUpRight,
    ExternalLink,
    FolderOpen,
    Monitor,
    Moon,
    Navigation,
    Sun,
  } from "@lucide/svelte";
  import { setMode } from "mode-watcher";
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";

  let outputDir = $state("");
  let currentTheme = $state<"light" | "dark" | "system">("system");
  let editorWindow = $state<"navigate" | "new-window">("navigate");

  onMount(() => {
    fetchSettings();
    const storedTheme = localStorage.getItem("mode-watcher-mode") as "light" | "dark" | "system" | null;
    if (storedTheme) currentTheme = storedTheme;
    const storedEditor = localStorage.getItem("recast-editor-window") as "navigate" | "new-window" | null;
    if (storedEditor) editorWindow = storedEditor;
  });

  async function fetchSettings() {
    try {
      outputDir = await getOutputDir();
    } catch (e) {
      toast.error(`Could not load settings: ${e}`);
    }
  }

  function updateTheme(theme: "light" | "dark" | "system") {
    setMode(theme);
    currentTheme = theme;
  }

  function updateEditorWindow(value: "navigate" | "new-window") {
    editorWindow = value;
    localStorage.setItem("recast-editor-window", value);
  }

  async function pickDirectory() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select Recording Directory",
    });
    if (selected && typeof selected === "string") {
      try {
        await setOutputDir(selected);
        outputDir = selected;
        toast.success("Output directory updated");
      } catch (e) {
        toast.error(`Could not set directory: ${e}`);
      }
    }
  }
</script>

<div class="flex h-full flex-col">
  <header class="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
    <div class="min-w-0">
      <h2 class="truncate text-[13px] font-semibold tracking-tight text-foreground">Settings</h2>
      <p class="truncate text-[11px] text-muted-foreground">Configure Recast defaults and preferences</p>
    </div>
  </header>

  <div class="flex-1 overflow-y-auto px-6 py-4">
    <div class="mx-auto flex max-w-2xl flex-col">
      <!-- Storage section -->
      <section class="border-b border-border py-4">
        <h3 class="mb-3 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Storage
        </h3>
        <div class="flex items-center gap-4 py-1.5">
          <label for="output-dir" class="w-44 shrink-0 text-[12px] font-medium text-foreground"
            >Output Directory</label
          >
          <div class="flex flex-1 items-center gap-2">
            <input
              id="output-dir"
              type="text"
              value={outputDir || "Default Temporary Directory"}
              title={outputDir}
              readonly
              class="h-8 flex-1 truncate rounded-md border border-input bg-input/30 px-2.5 text-[12px] text-muted-foreground outline-none"
            />
            <Button variant="secondary" size="sm" onclick={pickDirectory} class="h-8 gap-1.5">
              <FolderOpen size={13} />
              Change
            </Button>
          </div>
        </div>
      </section>

      <!-- Appearance section -->
      <section class="border-b border-border py-4">
        <h3 class="mb-3 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Appearance
        </h3>
        <div class="flex items-center gap-4 py-1.5">
          <span class="w-44 shrink-0 text-[12px] font-medium text-foreground">Theme</span>
          <div class="flex flex-1 items-center gap-1">
            <Button
              variant={currentTheme === "light" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-1.5"
              onclick={() => updateTheme("light")}
            >
              <Sun size={13} />
              Light
            </Button>
            <Button
              variant={currentTheme === "dark" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-1.5"
              onclick={() => updateTheme("dark")}
            >
              <Moon size={13} />
              Dark
            </Button>
            <Button
              variant={currentTheme === "system" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-1.5"
              onclick={() => updateTheme("system")}
            >
              <Monitor size={13} />
              System
            </Button>
          </div>
        </div>
      </section>

      <!-- Editor section -->
      <section class="border-b border-border py-4">
        <h3 class="mb-3 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Editor
        </h3>
        <div class="flex items-center gap-4 py-1.5">
          <span class="w-44 shrink-0 text-[12px] font-medium text-foreground">Window Behavior</span>
          <div class="flex flex-1 items-center gap-1">
            <Button
              variant={editorWindow === "navigate" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-1.5"
              onclick={() => updateEditorWindow("navigate")}
            >
              <Navigation size={13} />
              Navigate
            </Button>
            <Button
              variant={editorWindow === "new-window" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-1.5"
              onclick={() => updateEditorWindow("new-window")}
            >
              <ExternalLink size={13} />
              New Window
            </Button>
          </div>
        </div>
        <p class="ml-48 mt-1 text-[11px] text-muted-foreground">
          How the video editor opens when you click Edit.
        </p>
      </section>

      <!-- About section -->
      <section class="py-4">
        <h3 class="mb-3 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          About
        </h3>
        <div class="flex items-center gap-4 py-1.5">
          <span class="w-44 shrink-0 text-[12px] font-medium text-foreground">Version</span>
          <span class="flex-1 text-[12px] text-muted-foreground">{config.appName} v{config.appVersion}</span>
        </div>
        <div class="flex items-center gap-4 py-1.5">
          <span class="w-44 shrink-0 text-[12px] font-medium text-foreground">Links</span>
          <div class="flex flex-1 items-center gap-1">
            <Button href={config.website} target="_blank" variant="ghost" size="sm" class="h-8 gap-1.5">
              Website
              <ArrowUpRight size={13} />
            </Button>
            <Button href={config.github} target="_blank" variant="ghost" size="sm" class="h-8 gap-1.5">
              GitHub
              <ArrowUpRight size={13} />
            </Button>
          </div>
        </div>
      </section>
    </div>
  </div>
</div>
