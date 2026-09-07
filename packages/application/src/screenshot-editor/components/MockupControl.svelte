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
  import { PropRow } from "@recast/ui/prop-row";
  import { PropSelect } from "@recast/ui/prop-select";
  import { Segmented } from "@recast/ui/segmented";
  import { Input } from "@recast/ui/input";
  import type { MockupKind, MockupTheme } from "../types";

  let { editor }: MockupControlProps = $props();

  const showTheme = $derived(["window", "safari", "chrome"].includes(editor.mockup.kind));
  const showUrl = $derived(editor.mockup.kind === "safari" || editor.mockup.kind === "chrome");
</script>

<PanelSection variant="panel" title="Mockup" collapsible defaultOpen>
  <PropRow label="Kind">
    <PropSelect
      class="flex-1"
      label="Mockup kind"
      value={editor.mockup.kind}
      options={KINDS.map((k) => ({ value: k.value, label: k.label }))}
      onChange={(v) => editor.patchMockup({ kind: v as MockupKind })}
    />
  </PropRow>

  {#if showTheme}
    <PropRow label="Theme">
      <Segmented
        class="flex-1"
        options={[
          { value: "light", label: "Light" },
          { value: "dark", label: "Dark" },
        ]}
        value={editor.mockup.theme}
        onValueChange={(v) => editor.patchMockup({ theme: v as MockupTheme })}
        aria-label="Mockup theme"
      />
    </PropRow>
  {/if}

  {#if showUrl}
    <PropRow label="URL">
      <Input
        class="h-8 flex-1"
        value={editor.mockup.url}
        oninput={(e) => editor.patchMockup({ url: (e.currentTarget as HTMLInputElement).value })}
        placeholder="example.com"
        aria-label="Address bar URL"
      />
    </PropRow>
  {/if}
</PanelSection>
