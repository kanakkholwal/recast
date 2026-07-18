<script lang="ts">
  import { gdrive } from "$lib/stores/gdrive.svelte";
  import { HardDriveUpload, LoaderCircle, LogOut } from "@recast/icons";
  import { Button } from "@recast/ui/button";
  import { toast } from "@recast/ui/sonner";
  import { onMount } from "svelte";

  /**
   * Settings tile for the Google Drive connection. Modeled on
   * `CloudSignIn.svelte`, but the OAuth flow uses loopback redirect (no
   * device code to surface) so this is a much simpler two-state row:
   * connected (show email + Disconnect), or disconnected (Connect button).
   */

  let loadingStatus = $state(true);

  onMount(async () => {
    await gdrive.init();
    loadingStatus = false;
  });

  async function handleConnect() {
    try {
      await gdrive.connect();
      // Success surfaces via the gdrive:connected listener; only toast failures.
    } catch (e) {
      toast.error(`Couldn't connect Google Drive: ${e}`);
    }
  }

  async function handleDisconnect() {
    await gdrive.disconnect();
    toast.success("Disconnected from Google Drive.");
  }
</script>

<div class="px-4 py-3">
  {#if loadingStatus}
    <div class="flex items-center gap-2 text-[11.5px] text-muted-foreground">
      <LoaderCircle class="size-3.5 animate-spin" />
      <span>Checking Drive connection…</span>
    </div>
  {:else if gdrive.connected}
    <div class="flex items-center justify-between gap-3">
      <div class="flex min-w-0 items-center gap-3">
        <div
          class="flex size-9 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary ring-1 ring-inset ring-primary/30"
        >
          <HardDriveUpload class="size-4" />
        </div>
        <div class="min-w-0">
          <div class="text-[12px] font-semibold text-foreground">
            Connected to Google Drive
          </div>
          <div class="truncate text-[11px] text-muted-foreground">
            {gdrive.email ?? "Your Google account"}
          </div>
        </div>
      </div>
      <Button
        variant="ghost"
        size="sm"
        class="h-8 shrink-0 gap-1.5"
        onclick={handleDisconnect}
      >
        <LogOut class="size-3.5" />
        <span class="text-[11.5px]">Disconnect</span>
      </Button>
    </div>
  {:else}
    <div class="flex items-center justify-between gap-3">
      <div class="min-w-0">
        <div class="text-[12px] font-semibold text-foreground">
          Google Drive
        </div>
        <div class="text-[11px] text-muted-foreground">
          Connect to upload your exports to a private /Recast/ folder in your
          Drive. Recast only sees files it creates here.
        </div>
      </div>
      <Button
        size="sm"
        class="h-8 shrink-0 gap-1.5"
        disabled={gdrive.connecting}
        onclick={handleConnect}
      >
        {#if gdrive.connecting}
          <LoaderCircle class="size-3.5 animate-spin" />
          <span class="text-[11.5px]">Connecting…</span>
        {:else}
          <HardDriveUpload class="size-3.5" />
          <span class="text-[11.5px]">Connect</span>
        {/if}
      </Button>
    </div>
  {/if}
</div>
