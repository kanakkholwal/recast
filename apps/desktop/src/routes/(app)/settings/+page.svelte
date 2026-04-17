<script lang="ts">
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
  import { Button } from "@recast/ui/button";
  import { toast } from "@recast/ui/sonner";
  import { setMode } from "@recast/ui/theme";
  import { onMount } from "svelte";

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

<div class="flex h-full flex-col font-sans select-none">
  <header class="flex items-center justify-between gap-3 border-b border-border-subtle px-8 py-5 shrink-0">
    <div class="min-w-0 space-y-0.5">
      <h2 class="truncate text-[15px] font-semibold tracking-tight text-foreground">Settings</h2>
      <p class="truncate text-[11px] font-medium text-foreground/40 uppercase tracking-widest">Configure Recast defaults and preferences</p>
    </div>
  </header>

  <div class="flex-1 overflow-y-auto px-8 py-6 scrollbar-transparent">
    <div class="mx-auto flex max-w-2xl flex-col gap-6">
      <!-- Storage section -->
      <section class="bg-foreground/2 border border-border-subtle rounded-3xl p-6">
        <h3 class="mb-5 text-[10px] font-semibold uppercase tracking-[0.15em] text-foreground/30">
          Storage
        </h3>
        <div class="flex items-center gap-6">
          <label for="output-dir" class="w-40 shrink-0 text-[13px] font-medium text-foreground/70"
            >Output Directory</label
          >
          <div class="flex flex-1 items-center gap-3">
            <input
              id="output-dir"
              type="text"
              value={outputDir || "Default Temporary Directory"}
              title={outputDir}
              readonly
              class="h-9 flex-1 truncate rounded-xl border border-border-subtle bg-foreground/1 px-3.5 text-[12px] text-foreground/40 outline-none"
            />
            <Button variant="secondary" size="sm" onclick={pickDirectory} class="h-9 gap-2 rounded-xl border-border-subtle bg-background shadow-craft-sm hover:bg-foreground/2">
              <FolderOpen size={14} />
              <span class="text-[12px] font-medium">Change</span>
            </Button>
          </div>
        </div>
      </section>

      <!-- Appearance section -->
      <section class="bg-foreground/[0.02] border border-border-subtle rounded-3xl p-6">
        <h3 class="mb-5 text-[10px] font-semibold uppercase tracking-[0.15em] text-foreground/30">
          Appearance
        </h3>
        <div class="flex items-center gap-6">
          <span class="w-40 shrink-0 text-[13px] font-medium text-foreground/70">Theme</span>
          <div class="flex flex-1 items-center gap-1.5 p-1 bg-foreground/[0.03] rounded-2xl">
            <Button
              variant={currentTheme === "light" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-2 rounded-xl {currentTheme === 'light' ? 'bg-background shadow-craft-sm ring-1 ring-border-subtle' : 'text-foreground/40 hover:text-foreground/60'}"
              onclick={() => updateTheme("light")}
            >
              <Sun size={14} strokeWidth={2} />
              <span class="text-[12px] font-medium">Light</span>
            </Button>
            <Button
              variant={currentTheme === "dark" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-2 rounded-xl {currentTheme === 'dark' ? 'bg-background shadow-craft-sm ring-1 ring-border-subtle' : 'text-foreground/40 hover:text-foreground/60'}"
              onclick={() => updateTheme("dark")}
            >
              <Moon size={14} strokeWidth={2} />
              <span class="text-[12px] font-medium">Dark</span>
            </Button>
            <Button
              variant={currentTheme === "system" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-2 rounded-xl {currentTheme === 'system' ? 'bg-background shadow-craft-sm ring-1 ring-border-subtle' : 'text-foreground/40 hover:text-foreground/60'}"
              onclick={() => updateTheme("system")}
            >
              <Monitor size={14} strokeWidth={2} />
              <span class="text-[12px] font-medium">System</span>
            </Button>
          </div>
        </div>
      </section>

      <!-- Editor section -->
      <section class="bg-foreground/[0.02] border border-border-subtle rounded-3xl p-6">
        <h3 class="mb-5 text-[10px] font-semibold uppercase tracking-[0.15em] text-foreground/30">
          Editor
        </h3>
        <div class="flex items-center gap-6">
          <span class="w-40 shrink-0 text-[13px] font-medium text-foreground/70">Window Behavior</span>
          <div class="flex flex-1 items-center gap-1.5 p-1 bg-foreground/[0.03] rounded-2xl">
            <Button
              variant={editorWindow === "navigate" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-2 rounded-xl {editorWindow === 'navigate' ? 'bg-background shadow-craft-sm ring-1 ring-border-subtle' : 'text-foreground/40 hover:text-foreground/60'}"
              onclick={() => updateEditorWindow("navigate")}
            >
              <Navigation size={14} strokeWidth={2} />
              <span class="text-[12px] font-medium">Navigate</span>
            </Button>
            <Button
              variant={editorWindow === "new-window" ? "default_soft" : "ghost"}
              size="sm"
              class="h-8 flex-1 gap-2 rounded-xl {editorWindow === 'new-window' ? 'bg-background shadow-craft-sm ring-1 ring-border-subtle' : 'text-foreground/40 hover:text-foreground/60'}"
              onclick={() => updateEditorWindow("new-window")}
            >
              <ExternalLink size={14} strokeWidth={2} />
              <span class="text-[12px] font-medium">New Window</span>
            </Button>
          </div>
        </div>
        <p class="ml-46 mt-2.5 text-[11px] font-medium text-foreground/30">
          How the video editor opens when you click Edit.
        </p>
      </section>

      <!-- About section -->
      <section class="bg-foreground/2 border border-border-subtle rounded-3xl p-6">
        <h3 class="mb-5 text-[10px] font-semibold uppercase tracking-[0.15em] text-foreground/30">
          About
        </h3>
        <div class="space-y-4">
          <div class="flex items-center gap-6">
            <span class="w-40 shrink-0 text-[13px] font-medium text-foreground/70">Version</span>
            <span class="flex-1 text-[12px] font-medium text-foreground/40">{config.appName} v{config.appVersion}</span>
          </div>
          <div class="flex items-center gap-6 pt-2 border-t border-border-subtle">
            <span class="w-40 shrink-0 text-[13px] font-medium text-foreground/70">Links</span>
            <div class="flex flex-1 items-center gap-2">
              <Button href={config.website} target="_blank" variant="ghost" size="sm" class="h-8 gap-2 rounded-xl text-foreground/40 hover:text-foreground transition-all">
                <span class="text-[12px] font-medium">Website</span>
                <ArrowUpRight size={13} strokeWidth={2.5} />
              </Button>
              <Button href={config.github} target="_blank" variant="ghost" size="sm" class="h-8 gap-2 rounded-xl text-foreground/40 hover:text-foreground transition-all">
                <span class="text-[12px] font-medium">GitHub</span>
                <ArrowUpRight size={13} strokeWidth={2.5} />
              </Button>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</div>
