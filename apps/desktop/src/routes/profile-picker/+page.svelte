<script lang="ts">
  import {
    Camera,
    Check,
    Mic,
    SlidersHorizontal as SlidersIcon,
    Sparkles,
    Star,
    Volume2,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cn } from "@recast/ui/utils";
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  import type { RecordingProfile } from "$lib/profiles";
  import { profilesStore } from "$lib/stores/profiles.svelte";

  const params = new URLSearchParams(window.location.search);
  const initialSelected = params.get("selected") ?? null;

  let highlightedId = $state<string | null>(initialSelected);

  onMount(() => {
    profilesStore.hydrate();
    if (!highlightedId) {
      const def = profilesStore.default();
      if (def) highlightedId = def.id;
    }
  });

  function selectProfile(profile: RecordingProfile) {
    void emit("profile-selected", { id: profile.id });
    getCurrentWindow().close();
  }

  function closeWindow() {
    getCurrentWindow().close();
  }

  function moveHighlight(delta: 1 | -1) {
    const list = profilesStore.profiles;
    if (list.length === 0) return;
    const idx = list.findIndex((p) => p.id === highlightedId);
    const next = list[(idx + delta + list.length) % list.length];
    highlightedId = next.id;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      closeWindow();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      moveHighlight(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      moveHighlight(-1);
    } else if (e.key === "Enter" && highlightedId) {
      const target = profilesStore.findById(highlightedId);
      if (target) {
        e.preventDefault();
        selectProfile(target);
      }
    } else if ((e.metaKey || e.ctrlKey) && /^[1-8]$/.test(e.key)) {
      const idx = parseInt(e.key, 10) - 1;
      const target = profilesStore.profiles[idx];
      if (target) {
        e.preventDefault();
        selectProfile(target);
      }
    }
  }

  function summarize(profile: RecordingProfile): string[] {
    const out: string[] = [];
    if (profile.systemAudio) out.push("Audio");
    if (profile.microphone)
      out.push(profile.micLabel ? `Mic: ${profile.micLabel}` : "Mic");
    if (profile.camera)
      out.push(profile.cameraLabel ? `Cam: ${profile.cameraLabel}` : "Camera");
    if (out.length === 0) out.push("Silent capture");
    return out;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="group/root flex h-screen w-full flex-col overflow-hidden select-none rounded-2xl border border-border-subtle bg-card backdrop-blur-3xl"
>
  <!-- Header -->
  <header
    class="flex items-center justify-between border-b border-border-subtle px-4 h-10 shrink-0"
    data-tauri-drag-region
  >
    <div class="flex items-center gap-2">
      <SlidersIcon size={11} class="text-muted-foreground" />
      <span
        class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
      >
        Switch profile
      </span>
    </div>
    <Button
      onclick={closeWindow}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
      size="icon-sm"
      variant="ghost"
      class="opacity-0 group-hover/root:opacity-100 transition-opacity"
      title="Close (Esc)"
    >
      <X size={11} strokeWidth={2.5} />
    </Button>
  </header>

  <!-- Profile list -->
  <div class="flex-1 overflow-y-auto px-2 py-2 scrollbar-transparent">
    {#if profilesStore.profiles.length === 0}
      <div
        class="flex flex-col items-center justify-center h-40 gap-2 rounded-md border border-dashed border-border bg-card/40"
      >
        <SlidersIcon size={18} class="text-muted-foreground" />
        <p class="text-[11px] font-medium text-foreground">No profiles yet</p>
        <p class="text-[10px] text-muted-foreground">
          Open the main window → Profiles to create one.
        </p>
      </div>
    {:else}
      <div class="flex flex-col gap-0.5">
        {#each profilesStore.profiles as profile, i (profile.id)}
          {@const active = highlightedId === profile.id}
          <button
            type="button"
            onclick={() => selectProfile(profile)}
            onmouseenter={() => (highlightedId = profile.id)}
            onmousedown={(e) => e.stopPropagation()}
            class={cn(
              "group flex items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
              "focus:outline-none focus:ring-1 focus:ring-ring",
              active
                ? "bg-primary/10 text-foreground"
                : "text-foreground/80 hover:bg-muted/60",
            )}
          >
            <div
              class={cn(
                "size-7 shrink-0 rounded-md ring-1 ring-inset flex items-center justify-center",
                profile.isDefault
                  ? "bg-warning/10 text-warning ring-warning/30"
                  : active
                    ? "bg-primary text-primary-foreground ring-primary/40"
                    : "bg-muted text-muted-foreground ring-border/40",
              )}
            >
              {#if profile.isDefault}
                <Star size={11} />
              {:else}
                <SlidersIcon size={11} />
              {/if}
            </div>

            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <span
                  class="truncate text-[11.5px] font-semibold leading-tight text-foreground"
                >
                  {profile.name}
                </span>
                {#if profile.isDefault}
                  <Sparkles
                    size={9}
                    class="shrink-0 text-warning"
                    aria-label="Default profile"
                  />
                {/if}
              </div>
              <div
                class="mt-0.5 flex items-center gap-1 text-[10px] font-medium text-muted-foreground"
              >
                {#if profile.systemAudio}
                  <Volume2 size={9} class="shrink-0" />
                {/if}
                {#if profile.microphone}
                  <Mic size={9} class="shrink-0" />
                {/if}
                {#if profile.camera}
                  <Camera size={9} class="shrink-0" />
                {/if}
                <span class="truncate">{summarize(profile).join(" · ")}</span>
              </div>
            </div>

            {#if i < 8}
              <kbd
                class={cn(
                  "rounded border border-border/40 px-1 text-[9.5px] font-mono",
                  active ? "bg-card text-foreground" : "bg-muted/40 text-muted-foreground",
                )}
              >⌘{i + 1}</kbd>
            {/if}
            {#if profile.id === initialSelected}
              <Check
                size={12}
                strokeWidth={3}
                class="shrink-0 text-primary"
              />
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Footer hint -->
  <footer
    class="flex items-center justify-between border-t border-border-subtle bg-card/50 px-3 h-9 shrink-0"
  >
    <span class="text-[10px] font-medium text-muted-foreground/80">
      ↑↓ Enter · ⌘1-⌘8 · Esc
    </span>
    <Button
      onclick={closeWindow}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
      variant="ghost"
      size="xs"
    >
      Cancel
    </Button>
  </footer>
</div>

<style>
  :global(html) {
    background: transparent !important;
    scrollbar-width: none;
    scrollbar-gutter: auto !important;
    overflow: hidden;
  }
  :global(body) {
    background: transparent !important;
    overflow: hidden;
    margin: 0;
  }
  :global(html::-webkit-scrollbar),
  :global(body::-webkit-scrollbar) {
    width: 0;
    height: 0;
    display: none;
  }
</style>
