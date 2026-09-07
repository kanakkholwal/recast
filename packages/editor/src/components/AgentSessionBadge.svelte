<script lang="ts">
import { Sparkles } from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as Popover from "@recast/ui/popover";
import { agentSession } from "../lib/agent/session.svelte";

let open = $state(false);

const holder = $derived(agentSession.session.writerId || "an agent");
// Newest first: the last thing the agent did is what the user is looking for.
const entries = $derived([...agentSession.activity].reverse());

function clockOf(atMs: number): string {
	return new Date(atMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

async function takeOver() {
	open = false;
	await agentSession.takeOver();
}
</script>

{#if agentSession.active}
  <Popover.Root {open} onOpenChange={(v) => (open = v)}>
    <Popover.Trigger>
      {#snippet child({ props })}
        <button
          {...props as Record<string, unknown>}
          type="button"
          class="flex items-center gap-1.5 rounded-full border bg-muted px-2.5 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-muted/70 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          aria-label="An agent is editing this project. Open details."
        >
          <span class="relative flex size-1.5">
            <!-- Pulse is decorative; reduced-motion users get the static dot. -->
            <span
              class="absolute inline-flex size-full animate-ping rounded-full bg-foreground/50 motion-reduce:hidden"
            ></span>
            <span class="relative inline-flex size-1.5 rounded-full bg-foreground/70"></span>
          </span>
          <Sparkles class="size-3" aria-hidden="true" />
          Agent editing
        </button>
      {/snippet}
    </Popover.Trigger>
    <Popover.Content align="end" sideOffset={6} class="w-72 p-0">
      <div class="border-b px-3 py-2">
        <p class="text-[12px] font-medium">Agent session</p>
        <p class="text-[11px] text-muted-foreground">
          Editing is paused while {holder} works. You can still play and scrub.
        </p>
      </div>

      <div class="max-h-56 overflow-y-auto px-3 py-2">
        {#if entries.length === 0}
          <p class="py-2 text-[11px] text-muted-foreground">No changes yet.</p>
        {:else}
          <ul class="flex flex-col gap-1.5">
            {#each entries as entry (entry.id)}
              <li class="flex items-baseline justify-between gap-2 text-[11px]">
                <span>{entry.summary}</span>
                <span class="shrink-0 tabular-nums text-muted-foreground">
                  {clockOf(entry.atMs)}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      {#if agentSession.canTakeOver}
        <div class="border-t p-2">
          <Button variant="secondary" size="sm" class="w-full" onclick={takeOver}>
            Take over
          </Button>
          <p class="px-1 pt-1.5 text-[10px] text-muted-foreground">
            Ends the agent's session and returns control to you.
          </p>
        </div>
      {/if}
    </Popover.Content>
  </Popover.Root>
{/if}
