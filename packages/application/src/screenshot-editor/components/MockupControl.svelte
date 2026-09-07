<script lang="ts" module>
import type { ScreenshotEditorState } from "../editor.svelte";

export interface MockupControlProps {
	editor: ScreenshotEditorState;
}

const KINDS = [
	{ value: "none", label: "None" },
	{ value: "window", label: "Window" },
	{ value: "safari", label: "Safari" },
	{ value: "chrome", label: "Chrome" },
	{ value: "phone", label: "Phone" },
	{ value: "tablet", label: "Tablet" },
] as const;
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { Segmented } from "@recast/ui/segmented";
  import { Input } from "@recast/ui/input";
  import type { MockupKind, MockupTheme } from "../types";

  let { editor }: MockupControlProps = $props();

  const showTheme = $derived(["window", "safari", "chrome"].includes(editor.mockup.kind));
  const showUrl = $derived(editor.mockup.kind === "safari" || editor.mockup.kind === "chrome");
</script>

<PanelSection title="Mockup">
  <div class="grid grid-cols-3 gap-1.5">
    {#each KINDS as k (k.value)}
      <button
        type="button"
        class="rounded-lg border px-2 py-1.5 text-xs font-medium transition"
        class:bg-primary={editor.mockup.kind === k.value}
        class:text-primary-foreground={editor.mockup.kind === k.value}
        class:border-transparent={editor.mockup.kind === k.value}
        class:bg-card={editor.mockup.kind !== k.value}
        class:border-border={editor.mockup.kind !== k.value}
        class:hover:bg-muted={editor.mockup.kind !== k.value}
        aria-pressed={editor.mockup.kind === k.value}
        onclick={() => editor.patchMockup({ kind: k.value as MockupKind })}
      >
        {k.label}
      </button>
    {/each}
  </div>

  {#if showTheme}
    <Segmented
      options={[
        { value: "light", label: "Light" },
        { value: "dark", label: "Dark" },
      ]}
      value={editor.mockup.theme}
      onValueChange={(v) => editor.patchMockup({ theme: v as MockupTheme })}
      aria-label="Mockup theme"
    />
  {/if}

  {#if showUrl}
    <Input
      value={editor.mockup.url}
      oninput={(e) => editor.patchMockup({ url: (e.currentTarget as HTMLInputElement).value })}
      placeholder="example.com"
      aria-label="Address bar URL"
    />
  {/if}
</PanelSection>
