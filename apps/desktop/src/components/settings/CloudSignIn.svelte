<script lang="ts">
import { formatBytes } from "@recast/editor/lib/format/bytes";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import {
	ArrowUpRight,
	BarChart3,
	Check,
	ChevronsUpDown,
	Crown,
	LoaderCircle,
	Lock,
	LogOut,
	MessageSquare,
	Palette,
	ShieldAlert,
	Sparkles,
	Users,
	Video,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import RecastMark from "$components/recast-mark.svelte";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { toast } from "@recast/ui/sonner";
import { cn } from "@recast/ui/utils";
import { openUrl } from "@tauri-apps/plugin-opener";
import { onDestroy, onMount } from "svelte";
import {
	formatMemberSince,
	formatUserCode,
	initials,
	planLabel,
	roleLabel,
} from "./cloud-signin.logic";
import { CloudAuth } from "./cloud-signin.svelte";

// Recast Cloud sign-in state machine (see cloud-signin.svelte.ts). `view` is
// read through a local $derived so discriminated-union narrowing on it keeps
// working in the markup below.
const auth = new CloudAuth();
const view = $derived(auth.view);
const busy = $derived(auth.busy);
const inFlight = $derived(auth.inFlight);
const startSignIn = () => auth.startSignIn();
const signOut = () => auth.signOut();
const cancelSignIn = () => auth.cancelSignIn();

// Value-first sell for the signed-out state: show what Cloud unlocks before
// asking the user to authenticate.
const cloudBenefits = [
	{ icon: BarChart3, label: "Viewer analytics" },
	{ icon: MessageSquare, label: "Timestamped comments" },
	{ icon: Lock, label: "Password protection" },
	{ icon: Palette, label: "Custom branding" },
];

const dashboardUrl = "https://recast.nexonauts.com/dashboard/settings/profile";

async function openDashboard() {
	try {
		await openUrl(dashboardUrl);
	} catch (e) {
		toast.error(`Couldn't open browser: ${e}`);
	}
}

onMount(() => auth.start());
onDestroy(() => auth.dispose());
</script>

<div class="px-4 py-3">
  {#if view.kind === "loading"}
    <div class="flex items-center gap-2 text-[11.5px] text-muted-foreground">
      <LoaderCircle class="size-3.5 animate-spin" />
      <span>Checking sign-in…</span>
    </div>
  {:else if view.kind === "signed-in"}
    {@const planId = view.plan?.id ?? "free"}
    {@const isPaid = planId !== "free"}
    {@const memberSinceLabel = formatMemberSince(view.memberSince)}
    {@const shareCap = view.usage?.sharesLimit}
    {@const sharesActive = view.usage?.activeShares ?? 0}
    <div class="flex flex-col">
      <!-- Identity row: avatar + name/email + plan badge -->
      <div class="flex items-center gap-3 px-4 py-4">
        <div
          class="relative flex size-11 shrink-0 items-center justify-center overflow-hidden rounded-full bg-muted text-[13px] font-semibold text-foreground ring-1 ring-inset ring-border/50"
        >
          {#if view.image}
            <img
              src={view.image}
              alt={view.name ?? view.email ?? "Profile"}
              referrerpolicy="no-referrer"
              class="size-full object-cover"
            />
          {:else}
            {initials(view.name, view.email)}
          {/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <div class="truncate text-[13px] font-semibold text-foreground">
              {view.name ?? view.email ?? "Signed in"}
            </div>
            <span
              class={cn(
                "inline-flex shrink-0 items-center gap-1 rounded-full px-1.5 py-0.5 text-[9.5px] font-bold uppercase tracking-widest",
                isPaid
                  ? "bg-primary/10 text-primary ring-1 ring-inset ring-primary/30"
                  : "bg-muted text-muted-foreground ring-1 ring-inset ring-border/50",
              )}
            >
              {#if isPaid}
                <Crown class="size-2.5" />
              {/if}
              {view.plan?.name ?? "Free"}
            </span>
          </div>
          {#if view.name && view.email}
            <div class="truncate text-[11px] text-muted-foreground">
              {view.email}
            </div>
          {/if}
          {#if memberSinceLabel}
            <div class="mt-0.5 text-[10.5px] text-muted-foreground/70">
              Member since {memberSinceLabel}
            </div>
          {/if}
        </div>
        <Button
          variant="destructive_soft"
          size="sm"
          class="shrink-0"
          disabled={busy}
          onclick={signOut}
        >
          {#if inFlight === "sign-out"}
            <LoaderCircle class="animate-spin" />
            Signing out…
          {:else}
            <LogOut />
            Sign out
          {/if}
        </Button>
      </div>

      <!-- Default workspace, only meaningful when the user belongs to
				 more than one. Uploads target this workspace unless overridden
				 at share time. Backed by the shared cloudShare store (which the
				 share flow reads), persisted locally; it never changes the
				 web session's active org. -->
      {#if cloudShare.workspaces.length > 1}
        {@const active = cloudShare.activeWorkspace}
        <div
          class="flex items-center justify-between gap-3 border-t border-border/40 px-4 py-3"
        >
          <div class="flex min-w-0 items-center gap-2">
            <Users class="size-3.5 shrink-0 text-muted-foreground" />
            <div class="min-w-0">
              <div class="text-[11px] font-semibold text-foreground/85">
                Upload to
              </div>
              <div class="truncate text-[10.5px] text-muted-foreground">
                New shares are saved here
              </div>
            </div>
          </div>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger
              class={cn(
                "inline-flex h-8 max-w-[55%] items-center gap-1.5 rounded-md border border-border/60 bg-background/50 px-2.5 text-[11.5px] font-medium text-foreground transition-colors hover:bg-foreground/4",
              )}
            >
              <span class="truncate">{active?.name ?? "Select workspace"}</span>
              <ChevronsUpDown class="size-3 shrink-0 text-muted-foreground" />
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="end" class="min-w-60">
              <DropdownMenu.Label
                class="text-[10px] uppercase tracking-wide text-muted-foreground"
              >
                Workspaces
              </DropdownMenu.Label>
              {#each cloudShare.workspaces as ws (ws.id)}
                {@const selected = active?.id === ws.id}
                {@const isPaid = ws.plan !== "free"}
                <DropdownMenu.Item
                  class="gap-2 py-2"
                  onSelect={() => cloudShare.setWorkspace(ws.id)}
                >
                  <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span class="flex items-center gap-1.5">
                      <span class="truncate text-[12px] font-medium"
                        >{ws.name}</span
                      >
                      <span
                        class={cn(
                          "inline-flex shrink-0 items-center gap-0.5 rounded-full px-1.5 py-px text-[9px] font-bold uppercase tracking-wide",
                          isPaid
                            ? "bg-primary/10 text-primary ring-1 ring-inset ring-primary/30"
                            : "bg-muted text-muted-foreground ring-1 ring-inset ring-border/50",
                        )}
                      >
                        {#if isPaid}<Crown class="size-2" />{/if}
                        {planLabel(ws.plan)}
                      </span>
                    </span>
                    <span class="text-[10px] text-muted-foreground">
                      {roleLabel(ws.role)} · {ws.recastsCount}
                      {ws.recastsCount === 1 ? "recast" : "recasts"}
                    </span>
                  </span>
                  {#if selected}
                    <Check class="size-3.5 shrink-0 text-primary" />
                  {:else}
                    <span class="size-3.5 shrink-0"></span>
                  {/if}
                </DropdownMenu.Item>
              {/each}
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>
      {/if}

      <!-- Usage stats, only render if we got profile data back. The
				 fallback get-session path leaves `usage` null; rather than
				 stub zeros (which read as "you have nothing") we hide the
				 row entirely. -->
      {#if view.usage}
        <div
          class="grid grid-cols-3 divide-x divide-border/40 border-t border-border/40"
        >
          <div class="flex flex-col gap-0.5 px-4 py-3">
            <div
              class="flex items-center gap-1 text-[9.5px] font-semibold uppercase tracking-[0.12em] text-muted-foreground/70"
            >
              <Video class="size-2.5" />
              Recordings
            </div>
            <div class="text-[14px] font-semibold text-foreground">
              {view.usage.recordings}
            </div>
          </div>
          <div class="flex flex-col gap-0.5 px-4 py-3">
            <div
              class="text-[9.5px] font-semibold uppercase tracking-[0.12em] text-muted-foreground/70"
            >
              Storage
            </div>
            <div class="text-[14px] font-semibold text-foreground">
              {formatBytes(view.usage.storageBytes)}
            </div>
          </div>
          <div class="flex flex-col gap-0.5 px-4 py-3">
            <div
              class="text-[9.5px] font-semibold uppercase tracking-[0.12em] text-muted-foreground/70"
            >
              Active shares
            </div>
            <div class="text-[14px] font-semibold text-foreground">
              {sharesActive}{#if shareCap != null}
                <span class="text-[11px] font-medium text-muted-foreground"
                  >/{shareCap}</span
                >
              {/if}
            </div>
          </div>
        </div>
      {/if}

      <!-- Actions: upgrade CTA (free only) + manage account in browser -->
      <div class="flex flex-col gap-2 border-t border-border/40 px-4 py-3">
        <div class="flex flex-wrap items-center gap-2">
          {#if !isPaid}
            <Button
              size="sm"
              class="h-8 gap-1.5"
              onclick={() => openUrl("https://recast.li/pricing")}
            >
              <Sparkles class="size-3.5" />
              <span class="text-[11.5px]">Upgrade to Pro</span>
            </Button>
          {/if}
          <Button
            variant="outline"
            size="sm"
            class="h-8 gap-1.5"
            onclick={openDashboard}
          >
            <span class="text-[11.5px]">Manage account</span>
            <ArrowUpRight class="size-3 text-muted-foreground" />
          </Button>
          {#if view.plan?.cancelAtPeriodEnd && view.plan?.currentPeriodEnd}
            <span class="ml-auto text-[10.5px] text-warning">
              Ends {new Date(view.plan.currentPeriodEnd).toLocaleDateString()}
            </span>
          {/if}
        </div>
        {#if !isPaid}
          <p class="text-[10.5px] text-muted-foreground">
            Pro lifts your active-link limit.
          </p>
        {/if}
      </div>
    </div>
  {:else if view.kind === "waiting"}
    <div class="flex flex-col gap-3">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <div class="text-[12px] font-semibold text-foreground">
            Waiting for browser approval
          </div>
          <div class="text-[11px] text-muted-foreground">
            Approve the sign-in in the browser tab we opened.
          </div>
        </div>
        <Button
          variant="ghost"
          size="sm"
          class="h-8 gap-1.5 text-muted-foreground"
          onclick={cancelSignIn}
        >
          <span class="text-[11.5px]">Cancel</span>
        </Button>
      </div>
      <div
        class="flex items-center justify-between gap-3 rounded-lg border border-border/50 bg-background/50 px-3 py-2"
      >
        <span
          class="text-[10px] font-semibold uppercase tracking-[0.15em] text-muted-foreground"
        >
          Code
        </span>
        <span
          class="font-mono text-[13px] font-semibold tracking-[0.25em] text-foreground"
        >
          {formatUserCode(view.userCode)}
        </span>
      </div>
    </div>
  {:else if view.kind === "denied"}
    <div class="flex items-center justify-between gap-3">
      <div class="flex min-w-0 items-center gap-3">
        <div
          class="flex size-9 shrink-0 items-center justify-center rounded-xl bg-destructive/10 text-destructive ring-1 ring-inset ring-destructive/30"
        >
          <ShieldAlert class="size-4" />
        </div>
        <div class="min-w-0">
          <div class="text-[12px] font-semibold text-foreground">
            Sign-in denied
          </div>
          <div class="text-[11px] text-muted-foreground">
            Authorization was rejected in the browser.
          </div>
        </div>
      </div>
      <Button
        variant="secondary"
        size="sm"
        class="h-8 gap-1.5"
        disabled={busy}
        onclick={startSignIn}
      >
        {#if inFlight === "sign-in"}
          <LoaderCircle class="size-3.5 animate-spin" />
          <span class="text-[11.5px]">Signing in…</span>
        {:else}
          <RecastMark class="size-3.5" />
          <span class="text-[11.5px]">Try again</span>
        {/if}
      </Button>
    </div>
  {:else if view.kind === "expired"}
    <div class="flex items-center justify-between gap-3">
      <div class="flex min-w-0 items-center gap-3">
        <div
          class="flex size-9 shrink-0 items-center justify-center rounded-xl bg-warning/10 text-warning ring-1 ring-inset ring-warning/30"
        >
          <ShieldAlert class="size-4" />
        </div>
        <div class="min-w-0">
          <div class="text-[12px] font-semibold text-foreground">
            Code expired
          </div>
          <div class="text-[11px] text-muted-foreground">
            Take less than 30 minutes next time.
          </div>
        </div>
      </div>
      <Button
        variant="secondary"
        size="sm"
        class="h-8 gap-1.5"
        disabled={busy}
        onclick={startSignIn}
      >
        {#if inFlight === "sign-in"}
          <LoaderCircle class="size-3.5 animate-spin" />
          <span class="text-[11.5px]">Signing in…</span>
        {:else}
          <RecastMark class="size-3.5" />
          <span class="text-[11.5px]">Try again</span>
        {/if}
      </Button>
    </div>
  {:else}
    <div class="flex flex-col gap-3.5">
      <div class="min-w-0">
        <div class="text-[12.5px] font-semibold text-foreground">
          Turn any recording into a shareable link
        </div>
        <div class="mt-0.5 text-[11.5px] leading-relaxed text-muted-foreground">
          No re-upload. Cloud layers on top of the files you already have.
        </div>
      </div>
      <ul class="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {#each cloudBenefits as benefit (benefit.label)}
          {@const Icon = benefit.icon}
          <li class="flex items-center gap-2 text-[11.5px] text-foreground/85">
            <Icon class="size-3.5 shrink-0 text-primary" />
            <span>{benefit.label}</span>
          </li>
        {/each}
      </ul>
      <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
        <Button
          size="sm"
          class="h-8 gap-1.5"
          disabled={busy}
          onclick={startSignIn}
        >
          {#if inFlight === "sign-in"}
            <LoaderCircle class="size-3.5 animate-spin" />
            <span class="text-[11.5px]">Signing in…</span>
          {:else}
            <RecastMark class="size-3.5" />
            <span class="text-[11.5px]">Sign in to Recast Cloud</span>
          {/if}
        </Button>
        <span class="text-[10.5px] text-muted-foreground">
          The app never needs an account. Cloud is opt-in.
        </span>
      </div>
    </div>
  {/if}
</div>
