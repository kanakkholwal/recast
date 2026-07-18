<script lang="ts">
  import { diagnostics } from "$lib/logger/diagnostics.svelte";
  import { openLogDir } from "$lib/ipc";
  import { Button } from "@recast/ui/button";
  import { toast } from "@recast/ui/sonner";
  import { cn } from "@recast/ui/utils";
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
      class="flex items-center gap-1.5 text-[11px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
    >
      <ScrollText class="size-3 text-primary" />
      Diagnostics
    </h2>
    <p class="mt-0.5 text-[11px] text-muted-foreground/80">
      Detailed logs for troubleshooting. Turn this on while reproducing a bug,
      then send the log folder to support.
    </p>
  </div>

  <div
    class="overflow-hidden rounded-xl border border-border/60 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur"
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
      <button
        type="button"
        role="switch"
        aria-label="Diagnostic logging"
        aria-checked={diagnostics.enabled}
        onclick={toggleDiagnostics}
        class={cn(
          "flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors",
          diagnostics.enabled
            ? "bg-primary"
            : "bg-input ring-1 ring-inset ring-border/50",
        )}
      >
        <span
          class={cn(
            "size-4 rounded-full bg-card shadow-sm transition-transform",
            diagnostics.enabled ? "translate-x-4.5" : "translate-x-0.5",
          )}
        ></span>
      </button>
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
