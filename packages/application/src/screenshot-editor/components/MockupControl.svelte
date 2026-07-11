<script lang="ts" module>
  import type { ScreenshotEditorState } from "../editor.svelte";

  export interface MockupControlProps {
    editor: ScreenshotEditorState;
  }
</script>

<script lang="ts">
  import { PanelSection } from "@recast/ui/panel-section";
  import { Segmented } from "@recast/ui/segmented";
  import { Input } from "@recast/ui/input";
  import type { MockupKind, MockupTheme } from "../types";

  let { editor }: MockupControlProps = $props();

  const showChrome = $derived(editor.mockup.kind !== "none");
  const showUrl = $derived(editor.mockup.kind === "safari" || editor.mockup.kind === "chrome");
</script>

<PanelSection title="Mockup">
  <Segmented
    options={[
      { value: "none", label: "None" },
      { value: "window", label: "Window" },
      { value: "safari", label: "Safari" },
      { value: "chrome", label: "Chrome" },
    ]}
    value={editor.mockup.kind}
    onValueChange={(v) => editor.patchMockup({ kind: v as MockupKind })}
    size="xs"
    aria-label="Window mockup"
  />

  {#if showChrome}
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
