<script lang="ts">
  import { ScreenshotEditor } from "@recast/application/screenshot-editor";
  import { toast } from "@recast/ui/sonner";
  import { SeoMeta } from "$lib/components";

  // Web has no native screen capture; the editor falls back to upload/paste/drop.
  function notify(message: string, kind: "success" | "error") {
    if (kind === "success") toast.success(message);
    else toast.error(message);
  }
</script>

<SeoMeta
  title="Free Screenshot Editor"
  description="Beautify a screenshot with a backdrop, padding, rounded corners, and shadow, then export at up to 4x or copy to your clipboard. Runs entirely in your browser."
  eyebrow="Tools"
/>

<main class="h-[calc(100dvh-4rem)] w-full">
  <ScreenshotEditor onnotify={notify} class="h-full" />
</main>
