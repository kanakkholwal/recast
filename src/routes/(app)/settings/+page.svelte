<script lang="ts">
  import { Button } from "$components/ui/button";
  import { ButtonGroup } from "$components/ui/button-group";
  import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
  } from "$components/ui/card";
  import { Input } from "$components/ui/input";
  import { Label } from "$components/ui/label";
  import { config } from "$constants/app";
  import { getOutputDir, setOutputDir } from "$lib/ipc";
  import { ArrowUpRight, ExternalLink, Monitor, Moon, Navigation, Sun } from "@lucide/svelte";
  import { setMode } from "mode-watcher";
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";

  let outputDir = $state("");
  let isEditingDir = $state(false);
  let newDirInput = $state("");
  let currentTheme = $state("system");
  let editorWindow = $state<"navigate" | "new-window">("navigate");

  onMount(() => {
    fetchSettings();
    const storedTheme = localStorage.getItem("mode-watcher-mode");
    if (storedTheme) {
      currentTheme = storedTheme;
    }
    const storedEditorBehavior = localStorage.getItem("recast-editor-window");
    if (storedEditorBehavior === "new-window") {
      editorWindow = "new-window";
    }
  });

  function updateTheme(newTheme: "light" | "dark" | "system") {
    setMode(newTheme);
    currentTheme = newTheme;
  }

  async function fetchSettings() {
    try {
      outputDir = await getOutputDir();
    } catch (e) {
      console.error(e);
    }
  }

  async function handleDirectoryChange() {
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
      } catch (e) {
        toast.error(`Could not set directory: ${e}`);
      }
    }
  }
</script>

<div
  class="flex-1 flex flex-col p-8 w-full max-w-3xl mx-auto animate-in fade-in slide-in-from-bottom-4 duration-500"
>
  <div class="mb-10 border-b pb-6">
    <h2 class="text-2xl font-bold tracking-tight text-foreground">Settings</h2>
    <p class="text-sm text-muted-foreground mt-1">
      Configure Recast defaults and preferences.
    </p>
  </div>

  <div class="grid grid-cols-1 gap-6">
    <section>
      <h3
        class="text-sm font-semibold uppercase tracking-wider text-muted-foreground mb-4"
      >
        Storage
      </h3>

      <Card>
        <CardHeader>
          <Label for="output-dir">Output Directory</Label>
          <CardDescription>
            Choose the folder where your Recast recordings are saved.
          </CardDescription>
        </CardHeader>
        <CardContent class="flex flex-col gap-2">
          <div class="flex items-center gap-3">
            <Input
              class="flex-1 px-3 py-2.5 rounded-lg truncate"
              title={outputDir}
              value={outputDir || "Default Temporary Directory"}
              readonly
              disabled
              id="output-dir"
            />
            <Button
              variant="secondary"
              type="button"
              onclick={handleDirectoryChange}
            >
              Change
            </Button>
          </div>
        </CardContent>
      </Card>
    </section>

    <section class="mt-4">
      <h3
        class="text-sm font-semibold uppercase tracking-wider text-muted-foreground mb-4"
      >
        Appearance
      </h3>

      <Card class="flex-row">
        <CardHeader class="flex-1">
          <CardTitle>Theme mode</CardTitle>
          <CardDescription>
            Customize the application's appearance.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <ButtonGroup aria-label="Theme mode">
            <Button
              variant={currentTheme === "light" ? "default_soft" : "secondary"}
              class="flex-1 flex gap-2"
              onclick={() => updateTheme("light")}
            >
              <Sun size={16} />
              Light
            </Button>
            <Button
              variant={currentTheme === "dark" ? "default_soft" : "secondary"}
              class="flex-1 flex gap-2"
              onclick={() => updateTheme("dark")}
            >
              <Moon size={16} />
              Dark
            </Button>
            <Button
              variant={currentTheme === "system" ? "default_soft" : "secondary"}
              class="flex-1 flex gap-2"
              onclick={() => updateTheme("system")}
            >
              <Monitor size={16} />
              System
            </Button>
          </ButtonGroup>
        </CardContent>
      </Card>
    </section>

    <section class="mt-4">
      <h3
        class="text-sm font-semibold uppercase tracking-wider text-muted-foreground mb-4"
      >
        Editor
      </h3>

      <Card class="flex-row">
        <CardHeader class="flex-1">
            <CardTitle>Window Behavior</CardTitle>
            <CardDescription>
              How the video editor opens when you click Edit.
            </CardDescription>
        </CardHeader>
        <CardContent>
          <ButtonGroup aria-label="Window Behavior">
            <Button
              variant={editorWindow === "navigate"
                ? "default_soft"
                : "secondary"}
              class="flex-1 flex gap-2"
              onclick={() => {
                editorWindow = "navigate";
                localStorage.setItem("recast-editor-window", "navigate");
              }}
            >
              <Navigation size={16} />
              Navigate
            </Button>
            <Button
              variant={editorWindow === "new-window"
                ? "default_soft"
                : "secondary"}
              class="flex-1 flex gap-2"
              onclick={() => {
                editorWindow = "new-window";
                localStorage.setItem("recast-editor-window", "new-window");
              }}
            >
              <ExternalLink size={16} />
              New Window
            </Button>
          </ButtonGroup>
        </CardContent>
      </Card>
    </section>

    <section class="mt-4">
      <h3
        class="text-sm font-semibold uppercase tracking-wider text-muted-foreground mb-4"
      >
        About
      </h3>

      <Card
      >
        <CardHeader>
          <CardTitle>Recast</CardTitle>
          <CardDescription>Version 0.0.1</CardDescription>
        </CardHeader>
        <CardContent>
            <ButtonGroup aria-label="About">
                <Button href={config.website} target="_blank" variant="link">
                    Website
                    <ArrowUpRight size={16} />
                </Button>
                <Button href={config.github} target="_blank" variant="link">
                    GitHub
                    <ArrowUpRight size={16} />
                </Button>
            </ButtonGroup>
        </CardContent>
      </Card>
    </section>
  </div>
</div>
