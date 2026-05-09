<script lang="ts">
  import {
    Camera,
    CameraOff,
    CheckCircle2,
    Copy,
    Mic,
    MicOff,
    MoreHorizontal,
    Pencil,
    Plus,
    Power,
    Search,
    SlidersHorizontal as SlidersIcon,
    Sparkles,
    Star,
    Trash2,
    Volume2,
    VolumeOff,
    X,
  } from "@lucide/svelte";
  import { Badge } from "@recast/ui/badge";
  import { Button } from "@recast/ui/button";
  import * as Dialog from "@recast/ui/dialog";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
  import { Kbd } from "@recast/ui/kbd";
  import * as Select from "@recast/ui/select";
  import { toast } from "@recast/ui/sonner";
  import * as Tooltip from "@recast/ui/tooltip";
  import { cn } from "@recast/ui/utils";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  import {
    enumerateCameras,
    type BrowserCamera,
  } from "$lib/camera/browser-devices";
  import { getAudioDevices, type AudioDeviceInfo } from "$lib/ipc";
  import type { RecordingProfile } from "$lib/profiles";
  import { profilesStore } from "$lib/stores/profiles.svelte";

  // mode = 'create' means draft is not yet in the store; mode = 'edit' means
  // draft mirrors an existing entry. Persistence only happens on Save.
  let mode = $state<"create" | "edit" | null>(null);
  let draft = $state<RecordingProfile | null>(null);
  let nameInputEl = $state<HTMLInputElement | null>(null);
  let query = $state("");

  // Device lists are loaded once on mount and refreshed each time the dialog
  // opens (devices come and go between recordings). Camera enumeration may
  // trigger a permission probe so we deliberately keep it off the critical
  // mount path — re-fetch when the user actually needs to pick.
  let mics = $state<AudioDeviceInfo[]>([]);
  let cameras = $state<BrowserCamera[]>([]);
  let devicesLoading = $state(false);

  // Device-aware combination math: cap is 2 × (2+#mics) × (2+#cams) — each
  // attached mic / camera unlocks a new "audio + this mic + that cam" slot.
  // With zero mics + zero cams, this collapses to the original 2³ = 8.
  const totalCombinations = $derived(
    profilesStore.maxCombinations(mics, cameras),
  );
  const remainingSlots = $derived(profilesStore.freeSlots(mics, cameras));
  const isFull = $derived(remainingSlots === 0);

  onMount(() => {
    profilesStore.hydrate();
    void loadDevices();

    window.addEventListener("keydown", handleGlobalShortcut);
    return () => window.removeEventListener("keydown", handleGlobalShortcut);
  });

  async function loadDevices() {
    devicesLoading = true;
    try {
      const [audioDevices, videoDevices] = await Promise.all([
        getAudioDevices().catch(() => [] as AudioDeviceInfo[]),
        enumerateCameras().catch(() => [] as BrowserCamera[]),
      ]);
      mics = audioDevices;
      cameras = videoDevices;
    } finally {
      devicesLoading = false;
    }
  }

  function addProfile() {
    if (isFull) {
      toast.info(
        `All ${totalCombinations} capability combinations are in use`,
      );
      return;
    }
    const combo = profilesStore.nextFreeCombo(mics, cameras);
    if (!combo) {
      toast.info(
        `All ${totalCombinations} capability combinations are in use`,
      );
      return;
    }
    // Resolve device labels for any specific id the combo picked, so the
    // dropdown opens pre-filled and the saved profile carries an identity.
    const micDevice = combo.micDeviceId
      ? mics.find((m) => m.id === combo.micDeviceId)
      : null;
    const camDevice = combo.cameraDeviceId
      ? cameras.find((c) => c.deviceId === combo.cameraDeviceId)
      : null;
    const draftProfile: RecordingProfile = {
      id: crypto.randomUUID(),
      name: `Profile ${profilesStore.profiles.length + 1}`,
      systemAudio: combo.systemAudio,
      microphone: combo.microphone,
      micDeviceId: combo.micDeviceId,
      micLabel: micDevice?.name ?? null,
      camera: combo.camera,
      cameraDeviceId: combo.cameraDeviceId,
      cameraLabel: camDevice?.label ?? null,
      isDefault: profilesStore.profiles.length === 0,
    };
    openDialog("create", draftProfile);
  }

  function duplicateProfile(profile: RecordingProfile) {
    if (isFull) {
      toast.info(
        `All ${totalCombinations} capability combinations are in use`,
      );
      return;
    }
    // Open as a draft — user must change capabilities before Save (the
    // duplicate-signature check would otherwise reject it).
    const copy: RecordingProfile = {
      ...profile,
      id: crypto.randomUUID(),
      name: `${profile.name} Copy`,
      isDefault: false,
    };
    openDialog("create", copy);
  }

  function openDialog(next: "create" | "edit", profile: RecordingProfile) {
    mode = next;
    draft = profile;
    void loadDevices();
    queueMicrotask(() => {
      nameInputEl?.focus();
      nameInputEl?.select();
    });
  }

  function deleteProfile(id: string) {
    const victim = profilesStore.findById(id);
    if (!victim) return;
    profilesStore.remove(id);
    toast.success(`Deleted "${victim.name}"`);
    if (draft?.id === id) {
      mode = null;
      draft = null;
    }
  }

  function setDefault(id: string) {
    profilesStore.setDefault(id);
    toast.success("Default profile updated");
  }

  function startEditing(profile: RecordingProfile) {
    openDialog("edit", { ...profile });
  }

  function finishEditing() {
    if (!mode || !draft) return;
    const trimmed = draft.name.trim();
    if (!trimmed) {
      toast.error("Name cannot be empty");
      return;
    }
    const next: RecordingProfile = { ...draft, name: trimmed };
    // If a capability is off, clear the matching device pointers so we don't
    // persist stale identity that won't be applied anyway.
    if (!next.microphone) {
      next.micDeviceId = null;
      next.micLabel = null;
    }
    if (!next.camera) {
      next.cameraLabel = null;
      next.cameraDeviceId = null;
    }

    const conflict = profilesStore.duplicateOf(next);
    if (conflict) {
      toast.error(
        `"${conflict.name}" already uses this combination — change a toggle or device`,
      );
      return;
    }

    if (mode === "create") {
      profilesStore.insert(next);
      toast.success("Profile created");
    } else {
      profilesStore.update(next);
      toast.success("Profile saved");
    }

    mode = null;
    draft = null;
  }

  function cancelEditing() {
    mode = null;
    draft = null;
  }

  function toggleDraft(
    field: "systemAudio" | "microphone" | "camera" | "isDefault",
  ) {
    if (!draft) return;
    if (field === "isDefault" && draft.isDefault) {
      const others = profilesStore.profiles.filter(
        (p) => p.id !== draft!.id,
      );
      if (others.length === 0) {
        toast.info("At least one profile must be default");
        return;
      }
    }
    const nextValue = !draft[field];
    draft = { ...draft, [field]: nextValue };

    // When turning a device-bound capability ON, prefill the saved device
    // from the current default so the dropdown isn't blank.
    if (field === "microphone" && nextValue && !draft.micDeviceId) {
      const def = mics.find((d) => d.isDefault) ?? mics[0];
      if (def) draft = { ...draft, micDeviceId: def.id, micLabel: def.name };
    }
    if (field === "camera" && nextValue && !draft.cameraDeviceId) {
      const def = cameras.find((c) => !c.isVirtual) ?? cameras[0];
      if (def)
        draft = {
          ...draft,
          cameraDeviceId: def.deviceId,
          cameraLabel: def.label,
        };
    }
  }

  function setMicSelection(id: string) {
    if (!draft) return;
    const dev = mics.find((m) => m.id === id);
    if (!dev) return;
    draft = { ...draft, micDeviceId: dev.id, micLabel: dev.name };
  }

  function setCameraSelection(id: string) {
    if (!draft) return;
    const dev = cameras.find((c) => c.deviceId === id);
    if (!dev) return;
    draft = { ...draft, cameraDeviceId: dev.deviceId, cameraLabel: dev.label };
  }

  function handleGlobalShortcut(e: KeyboardEvent) {
    const meta = e.metaKey || e.ctrlKey;
    if (!meta || e.shiftKey || e.altKey) return;
    if (mode) return;
    if (e.key.toLowerCase() === "n") {
      e.preventDefault();
      addProfile();
    }
  }

  function handleDialogKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      finishEditing();
    }
  }

  function enableProfileSystem() {
    profilesStore.setEnabled(true);
    toast.success("Profiles enabled");
  }

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return profilesStore.profiles;
    return profilesStore.profiles.filter((p) =>
      p.name.toLowerCase().includes(q),
    );
  });

  // Capability metadata for the per-card chip rail and dialog toggles.
  type Cap = {
    field: "systemAudio" | "microphone" | "camera";
    label: string;
    on: typeof Volume2;
    off: typeof VolumeOff;
  };
  const capabilities: Cap[] = [
    { field: "systemAudio", label: "System audio", on: Volume2, off: VolumeOff },
    { field: "microphone", label: "Microphone", on: Mic, off: MicOff },
    { field: "camera", label: "Camera", on: Camera, off: CameraOff },
  ];

  function summarize(profile: RecordingProfile): string {
    const parts = [
      profile.systemAudio && "System audio",
      profile.microphone &&
        (profile.micLabel ? `Mic: ${profile.micLabel}` : "Mic"),
      profile.camera &&
        (profile.cameraLabel
          ? `Cam: ${profile.cameraLabel}`
          : "Camera"),
    ].filter(Boolean) as string[];
    return parts.length === 0 ? "Silent capture" : parts.join(" · ");
  }
</script>

<div class="h-full overflow-y-auto scrollbar-transparent no-scrollbar">
  <div class="mx-auto flex max-w-5xl flex-col gap-8 px-6 py-10">
    <!-- Hero (matches the home page rhythm) -->
    <header
      in:fly={{ y: 12, duration: 320, easing: cubicOut }}
      class="flex flex-col gap-3"
    >
      <span
        class="inline-flex w-fit items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.15em] text-muted-foreground/80 backdrop-blur"
      >
        <SlidersIcon class="size-3 text-primary" />
        Profiles
      </span>
      <div class="flex items-end justify-between gap-3">
        <h1
          class="text-balance text-[28px] font-semibold leading-tight tracking-tight text-foreground md:text-[32px]"
        >
          <span
            class="bg-linear-to-r from-foreground to-foreground/55 bg-clip-text text-transparent"
          >
            {profilesStore.profiles.length === 0
              ? "No profiles yet"
              : profilesStore.profiles.length === 1
                ? "1 recording preset"
                : `${profilesStore.profiles.length} recording presets`}
          </span>
        </h1>
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <!--
                Wrap a span around the disabled button so pointer events still
                reach the trigger — disabled native buttons swallow hover.
              -->
              <span {...props as Record<string, unknown>} class="shrink-0">
                <Button
                  onclick={addProfile}
                  size="sm"
                  class="h-9 gap-1.5"
                  disabled={isFull}
                >
                  <Plus size={13} />
                  New profile
                  <Kbd
                    class="bg-primary-foreground/15 text-primary-foreground/90"
                    >⌘N</Kbd
                  >
                </Button>
              </span>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content
            side="bottom"
            class="max-w-xs text-[11px] leading-relaxed"
          >
            {#if isFull}
              <div class="flex flex-col gap-1">
                <span class="font-semibold text-foreground"
                  >No combinations left</span
                >
                <span class="text-muted-foreground">
                  Profiles are unique by audio · mic · camera, including which
                  specific device is picked. All {totalCombinations} combinations
                  for your current devices are taken — plug in another mic or
                  camera, or edit an existing profile to free a slot.
                </span>
              </div>
            {:else}
              New profile <Kbd
                class="bg-foreground/10 text-foreground/80 ml-1">⌘N</Kbd
              >
            {/if}
          </Tooltip.Content>
        </Tooltip.Root>
      </div>
      <p class="text-[12.5px] leading-relaxed text-muted-foreground">
        Save what to capture — system audio, mic, camera — and pick the default
        that loads on launch.
        {#if profilesStore.profiles.length > 0}
          <span class="text-muted-foreground/70">
            {remainingSlots === 0
              ? `All ${totalCombinations} combinations in use.`
              : `${remainingSlots} of ${totalCombinations} combinations free.`}
          </span>
        {/if}
      </p>
    </header>

    <!-- Disabled banner: profiles still configurable, but the recording
         panel won't auto-apply them until the system is re-enabled. -->
    {#if !profilesStore.enabled}
      <div
        in:fly={{ y: 8, duration: 240, easing: cubicOut }}
        class="flex items-center gap-3 rounded-xl border border-warning/30 bg-warning/10 px-4 py-3 shadow-(--shadow-craft-inset)"
        role="status"
      >
        <span
          class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-warning/15 text-warning ring-1 ring-inset ring-warning/30"
          aria-hidden="true"
        >
          <Power size={14} />
        </span>
        <div class="min-w-0 flex-1">
          <div class="text-[12.5px] font-semibold text-foreground">
            Profiles are off
          </div>
          <div class="text-[11px] text-muted-foreground">
            The recording panel won't auto-apply a default profile or show the
            switcher. Edits here are still saved for when you re-enable.
          </div>
        </div>
        <Button
          onclick={enableProfileSystem}
          variant="secondary"
          size="sm"
          class="h-8 shrink-0 gap-1.5"
        >
          <Power class="size-3.5" />
          <span class="text-[11.5px]">Enable</span>
        </Button>
      </div>
    {/if}

    <!-- Hero search bar (matches home page) -->
    <label
      in:fly={{ y: 8, duration: 280, delay: 60, easing: cubicOut }}
      class="group/search flex h-12 items-center gap-3 rounded-xl border border-border/60 bg-card/70 px-4 shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:border-border hover:bg-card hover:shadow-craft-sm focus-within:border-border focus-within:bg-card focus-within:shadow-craft-sm"
    >
      <Search
        class="size-4 shrink-0 text-muted-foreground/70 transition-colors group-focus-within/search:text-foreground group-hover/search:text-foreground"
      />
      <input
        bind:value={query}
        type="text"
        placeholder="Search profiles…"
        aria-label="Search profiles"
        class="flex-1 bg-transparent text-[13px] font-medium text-foreground placeholder:text-muted-foreground/80 focus:outline-none"
      />
      {#if query}
        <Button
          variant="ghost"
          size="icon-sm"
          class="size-6"
          onclick={() => (query = "")}
          title="Clear search"
        >
          <X class="size-3" />
        </Button>
      {/if}
    </label>

    <!-- Profile grid -->
    {#if filtered.length === 0}
      <div
        in:fade={{ duration: 200 }}
        class="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border/60 bg-card/40 p-12 text-center"
      >
        <div
          class="flex size-12 items-center justify-center rounded-xl bg-foreground/5 text-muted-foreground"
        >
          <SlidersIcon class="size-5" />
        </div>
        <div>
          <p class="text-[14px] font-semibold text-foreground">
            {query ? "No matches" : "No profiles yet"}
          </p>
          <p class="mt-1 text-[11.5px] text-muted-foreground">
            {query
              ? `Nothing matches "${query}".`
              : "Create a profile to save your recording presets."}
          </p>
        </div>
        {#if !query}
          <Button onclick={addProfile} size="xs" class="mt-1 gap-1.5">
            <Plus size={11} /> Create profile
          </Button>
        {/if}
      </div>
    {:else}
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {#each filtered as profile, i (profile.id)}
          <div
            in:fly={{
              y: 8,
              duration: 240,
              delay: Math.min(i * 40, 240),
              easing: cubicOut,
            }}
            class={cn(
              "group/card relative flex flex-col gap-3 rounded-xl border bg-card/70 p-4 shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:-translate-y-0.5 hover:shadow-craft-sm",
              profile.isDefault
                ? "border-warning/30 ring-1 ring-warning/15"
                : "border-border/50 hover:border-border",
            )}
          >
            <!-- Top row: name + default badge + actions -->
            <div class="flex items-start gap-2">
              <button
                type="button"
                onclick={() => startEditing(profile)}
                class="flex min-w-0 flex-1 items-center gap-2.5 text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/60 rounded-md"
              >
                <span
                  class={cn(
                    "flex size-9 shrink-0 items-center justify-center rounded-lg ring-1 ring-inset transition-colors",
                    profile.isDefault
                      ? "bg-warning/10 text-warning ring-warning/30"
                      : "bg-foreground/5 text-foreground ring-border/40",
                  )}
                >
                  {#if profile.isDefault}
                    <Star class="size-4" />
                  {:else}
                    <SlidersIcon class="size-4" />
                  {/if}
                </span>
                <div class="min-w-0 flex-1">
                  <div
                    class="truncate text-[13.5px] font-semibold text-foreground"
                  >
                    {profile.name}
                  </div>
                  <div
                    class="truncate text-[10.5px] text-muted-foreground/80"
                  >
                    {summarize(profile)}
                  </div>
                </div>
              </button>

              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button
                      {...props as Record<string, unknown>}
                      variant="ghost"
                      size="icon-sm"
                      class="size-7"
                      title="More actions"
                    >
                      <MoreHorizontal size={14} />
                    </Button>
                  {/snippet}
                </DropdownMenu.Trigger>
                <DropdownMenu.Content align="end" size="sm" class="w-44">
                  <DropdownMenu.Item onSelect={() => startEditing(profile)}>
                    <Pencil class="size-3" /> Edit profile
                    <DropdownMenu.Shortcut>
                      <Kbd>⌘R</Kbd>
                    </DropdownMenu.Shortcut>
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    disabled={isFull}
                    onSelect={() => duplicateProfile(profile)}
                  >
                    <Copy class="size-3" /> Duplicate
                    <DropdownMenu.Shortcut>
                      <Kbd>⌘D</Kbd>
                    </DropdownMenu.Shortcut>
                  </DropdownMenu.Item>
                  {#if !profile.isDefault}
                    <DropdownMenu.Item
                      onSelect={() => setDefault(profile.id)}
                    >
                      <CheckCircle2 class="size-3" /> Set as default
                    </DropdownMenu.Item>
                  {/if}
                  <DropdownMenu.Separator />
                  <DropdownMenu.Item
                    onSelect={() => deleteProfile(profile.id)}
                    class="text-destructive focus:bg-destructive/10 focus:text-destructive"
                  >
                    <Trash2 class="size-3" /> Delete
                  </DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            </div>

            <!-- Capability chip rail (native Badge) -->
            <div class="flex flex-wrap gap-1.5">
              {#each capabilities as cap (cap.field)}
                {@const on = profile[cap.field]}
                {@const Icon = on ? cap.on : cap.off}
                <Badge
                  variant={on ? "secondary" : "outline"}
                  class={cn(
                    "gap-1.5 px-2 text-[10px]",
                    on && "bg-primary/10 text-primary border-primary/25",
                    !on && "text-muted-foreground/70",
                  )}
                >
                  <Icon class="size-3" />
                  {cap.label}
                </Badge>
              {/each}
            </div>

            <!-- Footer: default toggle pill -->
            <div class="flex items-center justify-between pt-1">
              {#if profile.isDefault}
                <Badge
                  variant="outline"
                  class="gap-1 border-warning/25 bg-warning/10 text-warning"
                >
                  <Sparkles class="size-3" />
                  Default
                </Badge>
              {:else}
                <button
                  type="button"
                  onclick={() => setDefault(profile.id)}
                  class="text-[10.5px] font-medium text-muted-foreground transition-colors hover:text-foreground"
                >
                  Set as default
                </button>
              {/if}
              <Button
                variant="ghost"
                size="xs"
                onclick={() => startEditing(profile)}
                class="h-6 gap-1 px-1.5 text-[10.5px] text-muted-foreground hover:text-foreground"
              >
                <Pencil size={10} />
                Edit
              </Button>
            </div>
          </div>
        {/each}

        <!-- "New profile" call-to-card always at the end of the grid -->
        {#if !isFull}
          <button
            type="button"
            onclick={addProfile}
            in:fly={{
              y: 8,
              duration: 240,
              delay: Math.min(filtered.length * 40, 280),
              easing: cubicOut,
            }}
            class="group/add flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border/60 bg-card/30 p-6 text-center text-muted-foreground transition-all duration-200 hover:-translate-y-0.5 hover:border-primary/40 hover:bg-primary/5 hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
          >
            <span
              class="flex size-9 items-center justify-center rounded-lg bg-foreground/5 text-foreground transition-colors group-hover/add:bg-primary/10 group-hover/add:text-primary"
            >
              <Plus class="size-4" />
            </span>
            <div>
              <div class="text-[12.5px] font-semibold text-foreground">
                New profile
              </div>
              <div class="mt-0.5 text-[10.5px] text-muted-foreground/80">
                Save another preset
              </div>
            </div>
          </button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- Edit dialog -->
{#snippet toggleRow(
  field: "isDefault" | "systemAudio" | "microphone" | "camera",
  Icon: typeof Star,
  label: string,
  hint: string,
)}
  <button
    type="button"
    onclick={() => toggleDraft(field)}
    class="flex w-full items-center gap-3 px-5 py-3 text-left transition-colors hover:bg-foreground/4 focus-visible:bg-foreground/4 focus-visible:outline-none"
  >
    <span
      class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background/70 text-muted-foreground ring-1 ring-inset ring-border/40"
    >
      <Icon size={14} />
    </span>
    <span class="flex min-w-0 flex-1 flex-col gap-0.5">
      <span class="truncate text-[12.5px] font-semibold text-foreground"
        >{label}</span
      >
      <span class="truncate text-[11px] font-medium text-muted-foreground"
        >{hint}</span
      >
    </span>
    <span
      class={cn(
        "flex h-5 w-9 shrink-0 items-center rounded-full transition-colors",
        draft?.[field]
          ? "bg-primary"
          : "bg-input ring-1 ring-inset ring-border/50",
      )}
    >
      <span
        class={cn(
          "size-4 rounded-full bg-card shadow-sm transition-transform",
          draft?.[field] ? "translate-x-4.5" : "translate-x-0.5",
        )}
      ></span>
    </span>
  </button>
{/snippet}

{#snippet deviceRow(
  Icon: typeof Mic,
  label: string,
  hint: string,
  options: { value: string; label: string }[],
  selected: string | null,
  onSelect: (id: string) => void,
  emptyHint: string,
)}
  {@const currentLabel = options.find((o) => o.value === selected)?.label}
  <div class="flex flex-col gap-2 px-5 py-3 bg-muted/15">
    <div class="flex items-center gap-3">
      <span
        class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background/70 text-muted-foreground ring-1 ring-inset ring-border/40"
        aria-hidden="true"
      >
        <Icon size={14} />
      </span>
      <span class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span class="truncate text-[11.5px] font-semibold text-foreground/80">
          {label}
        </span>
        <span
          class="truncate text-[10.5px] font-medium text-muted-foreground/80"
        >
          {hint}
        </span>
      </span>
    </div>
    {#if options.length === 0}
      <div
        class="flex h-9 items-center justify-center rounded-lg border border-dashed border-border/60 bg-background/40 text-[11px] font-medium text-muted-foreground"
      >
        {devicesLoading ? "Loading devices…" : emptyHint}
      </div>
    {:else}
      <Select.Root
        type="single"
        value={selected ?? undefined}
        onValueChange={(v) => {
          if (typeof v === "string" && v.length > 0) onSelect(v);
        }}
      >
        <Select.Trigger
          class="h-9! w-full justify-between rounded-lg border border-border/50 bg-background/70 px-3 text-[11.5px] font-medium text-foreground hover:bg-background hover:border-border focus-visible:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/20"
          aria-label={label}
        >
          <span
            data-slot="select-value"
            class="flex min-w-0 flex-1 items-center gap-2"
          >
            <Icon class="size-3.5 shrink-0 text-muted-foreground" />
            <span class="truncate">
              {currentLabel ?? "Select a device…"}
            </span>
          </span>
        </Select.Trigger>
        <Select.Content sideOffset={6} class="max-h-64">
          {#each options as opt (opt.value)}
            <Select.Item
              value={opt.value}
              label={opt.label}
              class="text-[11.5px]"
            >
              <span class="truncate pr-4">{opt.label}</span>
            </Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
    {/if}
  </div>
{/snippet}

{#if mode !== null && draft}
  <Dialog.Root
    open={true}
    onOpenChange={(v) => {
      if (!v) cancelEditing();
    }}
  >
    <Dialog.Content
      showCloseButton={false}
      class="block! w-[calc(100%-2rem)] gap-0! overflow-hidden rounded-2xl p-0! ring-1 ring-border/60 shadow-(--shadow-craft-inset-strong) sm:max-w-md!"
    >
      <header
        class="flex items-center justify-between gap-3 border-b border-border/40 px-5 py-4"
      >
        <div class="min-w-0">
          <Dialog.Title
            class="text-[14px] font-semibold tracking-tight text-foreground"
          >
            {mode === "edit" ? "Edit Profile" : "New Profile"}
          </Dialog.Title>
          <Dialog.Description
            class="mt-0.5 text-[11px] font-medium text-muted-foreground"
          >
            Configure what to capture during recording.
          </Dialog.Description>
        </div>
        {#if draft.isDefault}
          <span
            class="inline-flex shrink-0 items-center gap-1 rounded-md border border-warning/20 bg-warning/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide text-warning"
          >
            <Star size={11} />
            Default
          </span>
        {/if}
      </header>

      <div class="border-b border-border/30 px-5 py-4">
        <label
          for="profile-name-input"
          class="mb-1.5 block text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground"
        >
          Name
        </label>
        <input
          id="profile-name-input"
          bind:this={nameInputEl}
          bind:value={draft.name}
          onkeydown={handleDialogKeydown}
          placeholder="My profile"
          class="h-9 w-full rounded-lg border border-border/50 bg-background px-3 text-[13px] font-medium text-foreground outline-none transition-all placeholder:text-muted-foreground/60 focus:border-primary/60 focus:ring-2 focus:ring-primary/20"
        />
      </div>

      <div class="divide-y divide-border/30">
        {@render toggleRow(
          "isDefault",
          Star,
          "Default profile",
          "Use this profile automatically on launch",
        )}
        {@render toggleRow(
          "systemAudio",
          Volume2,
          "System audio",
          "Capture sounds playing on your device",
        )}
        {@render toggleRow(
          "microphone",
          Mic,
          "Microphone",
          "Record your voice from the default input",
        )}
        {#if draft.microphone}
          {@render deviceRow(
            Mic,
            "Microphone device",
            "If unavailable at recording time, the system default is used.",
            mics.map((m) => ({
              value: m.id,
              label: m.name + (m.isDefault ? " (default)" : ""),
            })),
            draft.micDeviceId,
            setMicSelection,
            "No microphones detected",
          )}
        {/if}
        {@render toggleRow(
          "camera",
          Camera,
          "Camera",
          "Overlay webcam feed onto the recording",
        )}
        {#if draft.camera}
          {@render deviceRow(
            Camera,
            "Camera device",
            "Saved by name; falls back to first non-virtual cam if missing.",
            cameras.map((c) => ({
              value: c.deviceId,
              label: c.label + (c.isVirtual ? " (virtual)" : ""),
            })),
            draft.cameraDeviceId,
            setCameraSelection,
            "No cameras detected",
          )}
        {/if}
      </div>

      <footer
        class="flex items-center justify-between gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5"
      >
        {#if mode === "edit"}
          <Button
            variant="destructive_soft"
            size="xs"
            class="gap-1.5"
            onclick={() => {
              if (draft) deleteProfile(draft.id);
            }}
          >
            <Trash2 size={12} />
            Delete
          </Button>
        {:else}
          <span></span>
        {/if}
        <div class="flex items-center gap-2">
          <Button variant="ghost" size="xs" onclick={cancelEditing}
            >Cancel</Button
          >
          <Button
            variant="default"
            size="xs"
            class="gap-2"
            onclick={finishEditing}
          >
            Save
            <Kbd class="bg-primary-foreground/10 text-primary-foreground/80"
              >⌘↵</Kbd
            >
          </Button>
        </div>
      </footer>
    </Dialog.Content>
  </Dialog.Root>
{/if}
