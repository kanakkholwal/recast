<script lang="ts">
  import {
    ScreenshotEditor,
    type EditorImage,
  } from "@recast/application/screenshot-editor";
  import { toast } from "@recast/ui/sonner";
  import { captureScreenshot } from "$lib/ipc";

  // Native capture: grab the primary display, hand it to the editor as an image.
  // (A source picker can select a specific display/window/app later.)
  async function capture(): Promise<EditorImage | null> {
    const shot = await captureScreenshot();
    if (!shot.base64) throw new Error("capture returned no image data");
    return { src: shot.base64, width: shot.width, height: shot.height };
  }

  function notify(message: string, kind: "success" | "error") {
    if (kind === "success") toast.success(message);
    else toast.error(message);
  }
</script>

<div class="h-full min-h-0 w-full">
  <ScreenshotEditor oncapture={capture} onnotify={notify} class="h-full" />
</div>
