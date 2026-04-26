<script lang="ts">
  import PageShell from "$components/layout/PageShell.svelte";
  import SectionCard from "$components/layout/SectionCard.svelte";
  import SettingsRow from "$components/layout/SettingsRow.svelte";
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
  import { cn } from "@recast/ui/utils";
  import { onMount } from "svelte";

  type Theme = "light" | "dark" | "system";
  type EditorBehavior = "navigate" | "new-window";

  let outputDir = $state("");
  let currentTheme = $state<Theme>("system");
  let editorWindow = $state<EditorBehavior>("navigate");

  onMount(() => {
    fetchSettings();
    const storedTheme = localStorage.getItem("mode-watcher-mode") as Theme | null;
    if (storedTheme) currentTheme = storedTheme;
    const storedEditor = localStorage.getItem("recast-editor-window") as EditorBehavior | null;
    if (storedEditor) editorWindow = storedEditor;
  });

  async function fetchSettings() {
    try {
      outputDir = await getOutputDir();
    } catch (e) {
      toast.error(`Could not load settings: ${e}`);
    }
  }

  function updateTheme(theme: Theme) {
    setMode(theme);
    currentTheme = theme;
  }

  function updateEditorWindow(value: EditorBehavior) {
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

  const themes: { value: Theme; label: string; icon: typeof Sun }[] = [
    { value: "light", label: "Light", icon: Sun },
    { value: "dark", label: "Dark", icon: Moon },
    { value: "system", label: "System", icon: Monitor },
  ];

  const editorBehaviors: { value: EditorBehavior; label: string; icon: typeof Navigation }[] = [
    { value: "navigate", label: "Navigate", icon: Navigation },
    { value: "new-window", label: "New Window", icon: ExternalLink },
  ];
</script>

<PageShell title="Settings" subtitle="Configure Recast defaults and preferences">
  <div class="mx-auto flex w-full max-w-2xl flex-col gap-6 px-8 py-8">
    <SectionCard label="Storage">
      <SettingsRow label="Output Directory" hint="Where new recordings are saved.">
        <input
          type="text"
          value={outputDir || "Default Temporary Directory"}
          title={outputDir}
          readonly
          class="h-9 min-w-0 flex-1 truncate rounded-lg border border-border/40 bg-background/60 px-3 text-[12px] font-medium text-muted-foreground outline-none"
        />
        <Button variant="secondary" size="sm" onclick={pickDirectory}>
          <FolderOpen size={14} />
          <span class="text-[12px] font-medium">Change</span>
        </Button>
      </SettingsRow>
    </SectionCard>

    <SectionCard label="Appearance">
      <SettingsRow label="Theme" hint="Match your system or pick a fixed mode.">
        <div class="flex items-center gap-1 rounded-xl bg-muted/60 p-1 ring-1 ring-inset ring-border/40">
          {#each themes as t}
            {@const Icon = t.icon}
            <button
              type="button"
              onclick={() => updateTheme(t.value)}
              class={cn(
                "flex h-7 items-center gap-1.5 rounded-lg px-2.5 text-[11px] font-semibold transition-all",
                currentTheme === t.value
                  ? "bg-card text-foreground shadow-(--shadow-craft-inset)"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Icon size={13} />
              <span>{t.label}</span>
            </button>
          {/each}
        </div>
      </SettingsRow>
    </SectionCard>

    <SectionCard label="Editor">
      <SettingsRow label="Window Behavior" hint="How the editor opens when you click Edit.">
        <div class="flex items-center gap-1 rounded-xl bg-muted/60 p-1 ring-1 ring-inset ring-border/40">
          {#each editorBehaviors as b}
            {@const Icon = b.icon}
            <button
              type="button"
              onclick={() => updateEditorWindow(b.value)}
              class={cn(
                "flex h-7 items-center gap-1.5 rounded-lg px-2.5 text-[11px] font-semibold transition-all",
                editorWindow === b.value
                  ? "bg-card text-foreground shadow-(--shadow-craft-inset)"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Icon size={13} />
              <span>{b.label}</span>
            </button>
          {/each}
        </div>
      </SettingsRow>
    </SectionCard>

    <SectionCard label="About">
      <SettingsRow label="Version">
        <span class="text-[12px] font-medium text-muted-foreground">
          {config.appName} v{config.appVersion}
        </span>
      </SettingsRow>
      <SettingsRow label="Links">
        <Button
          href={config.website}
          target="_blank"
          variant="ghost"
          size="sm"
          class="h-8 gap-1.5 text-muted-foreground hover:text-foreground"
        >
          <span class="text-[12px] font-medium">Website</span>
          <ArrowUpRight size={12} />
        </Button>
        <Button
          href={config.github}
          target="_blank"
          variant="ghost"
          size="sm"
          class="h-8 gap-1.5 text-muted-foreground hover:text-foreground"
        >
          <span class="text-[12px] font-medium">GitHub</span>
          <ArrowUpRight size={12} />
        </Button>
      </SettingsRow>
    </SectionCard>
  </div>
</PageShell>
