<script lang="ts">
import { diagnostics } from "$lib/logger/diagnostics.svelte";
import { openLogDir } from "$lib/ipc";
import { Button } from "@recast/ui/button";
import { Switch } from "@recast/ui/switch";
import { toast } from "@recast/ui/sonner";
import { FolderOpen, ScrollText } from "@recast/icons";

let opening = $state(false);

function toggleDiagnostics() {
	const next = !diagnostics.enabled;
	diagnostics.set(next);
	toast.success(
		next
			? "Diagnostic logging on. Reproduce the issue, then open the logs folder."
			: "Diagnostic logging off",
	);
}

async function openLogs() {
	opening = true;
	try {
		await openLogDir();
	} catch (e) {
		toast.error(`Couldn't open the logs folder: ${e}`);
	} finally {
		opening = false;
	}
}
</script>

<section id="settings-diagnostics" class="flex flex-col gap-3">
  <div class="px-1">
    <h2
      class="flex items-center gap-1.5 text-[13px] font-semibold tracking-tight text-foreground"
    >
      <ScrollText class="size-3.5 text-muted-foreground" />
      Diagnostics
    </h2>
    <p class="mt-0.5 text-[11.5px] leading-relaxed text-muted-foreground">
      Detailed logs for troubleshooting. Turn this on while reproducing a bug,
      then send the log folder to support.
    </p>
  </div>

  <div
    class="overflow-hidden rounded-2xl border border-border/50 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
  >
    <div class="flex items-center justify-between gap-3 px-4 py-3">
      <div class="min-w-0">
        <div class="text-[12px] font-semibold text-foreground">
          Diagnostic logging
        </div>
        <div class="text-[11px] text-muted-foreground">
          Records what you do in the editor (which recast, selections, property
          changes, export settings) and backend processing to a local file.
          Nothing is uploaded. It stays on this machine until you share it.
        </div>
      </div>
      <Switch
        checked={diagnostics.enabled}
        onCheckedChange={() => toggleDiagnostics()}
        aria-label="Diagnostic logging"
      />
    </div>

    <div
      class="flex items-center justify-between gap-3 border-t border-border/40 px-4 py-3"
    >
      <div class="min-w-0">
        <div class="text-[12px] font-semibold text-foreground">Log files</div>
        <div class="text-[11px] text-muted-foreground">
          Open the folder to attach the logs to a support request.
        </div>
      </div>
      <Button
        variant="outline"
        size="xs"
        class="shrink-0 gap-1.5"
        disabled={opening}
        onclick={openLogs}
      >
        <FolderOpen class="size-3" />
        Open logs folder
      </Button>
    </div>
  </div>
</section>
