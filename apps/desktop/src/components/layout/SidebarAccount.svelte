<script lang="ts">
import { goto } from "$app/navigation";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import { ChevronRight, User } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import { onMount } from "svelte";
import { monogram, sharesMeter } from "./sidebar-account.logic";

let { open }: { open: boolean } = $props();

const signedIn = $derived(cloudShare.signedIn);
const name = $derived(signedIn ? (cloudShare.activeWorkspace?.name ?? "Recast Cloud") : "Sign in");
const sub = $derived(signedIn ? (cloudShare.planName ?? "Signed in") : "Sync & share your work");
const meter = $derived(
	signedIn ? sharesMeter(cloudShare.usage?.activeShares ?? 0, cloudShare.usage?.sharesLimit) : null,
);

// Populate the card regardless of which screen mounts first; init is cached.
onMount(() => void cloudShare.init());

const open_ = () => goto("/settings?tab=cloud");
</script>

{#if open}
  <button
    type="button"
    onclick={open_}
    class="group/acct flex w-full items-center gap-2.5 rounded-lg border border-border/40 bg-sidebar-accent/40 p-1.5 pr-2 text-left transition-colors duration-150 hover:bg-sidebar-accent"
    title={signedIn ? name : "Sign in to Recast Cloud"}
  >
    <span
      class={cn(
        "grid size-8 shrink-0 place-items-center rounded-md text-[11px] font-semibold",
        signedIn ? "bg-primary/15 text-primary" : "bg-foreground/5 text-muted-foreground",
      )}
    >
      {#if signedIn}
        {monogram(name)}
      {:else}
        <User size={15} />
      {/if}
    </span>
    <span class="min-w-0 flex-1">
      <span class="block truncate text-[12px] font-semibold text-foreground">{name}</span>
      <span class="block truncate text-[10.5px] text-muted-foreground">{sub}</span>
    </span>
    <ChevronRight
      class="size-3.5 shrink-0 text-muted-foreground/50 transition-transform duration-150 group-hover/acct:translate-x-0.5"
    />
  </button>
  {#if meter}
    <div class="px-1.5 pt-1.5">
      <div class="h-1 overflow-hidden rounded-full bg-foreground/10">
        <div class="h-full rounded-full bg-primary transition-[width] duration-300" style="width: {meter.pct}%"></div>
      </div>
      <p class="mt-1 text-[9.5px] text-muted-foreground/80">{meter.label}</p>
    </div>
  {/if}
{:else}
  <button
    type="button"
    onclick={open_}
    aria-label={signedIn ? name : "Sign in"}
    title={signedIn ? name : "Sign in"}
    class={cn(
      "mx-auto grid size-8 place-items-center rounded-md text-[11px] font-semibold transition-colors duration-150 motion-safe:active:scale-95",
      signedIn ? "bg-primary/15 text-primary hover:bg-primary/25" : "bg-foreground/5 text-muted-foreground hover:text-foreground",
    )}
  >
    {#if signedIn}
      {monogram(name)}
    {:else}
      <User size={15} />
    {/if}
  </button>
{/if}
