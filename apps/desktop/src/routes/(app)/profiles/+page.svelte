<script lang="ts">
  import {
    Camera,
    CheckCircle2,
    Copy,
    Info,
    Mic,
    MicOff,
    MoreHorizontal,
    Pencil,
    Plus,
    Power,
    Search,
    SlidersHorizontal as SlidersIcon,
    Star,
    Timer,
    Trash2,
    VideoOff,
    Volume2,
    VolumeX,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { Cutout } from "@recast/ui/cutout";
  import * as Dialog from "@recast/ui/dialog";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
  import { Kbd } from "@recast/ui/kbd";
  import * as Select from "@recast/ui/select";
  import { Segmented, type SegmentedOption } from "@recast/ui/segmented";
  import { toast } from "@recast/ui/sonner";
  import { Switch } from "@recast/ui/switch";
  import { cn } from "@recast/ui/utils";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  import {
    enumerateCameras,
    type BrowserCamera,
  } from "$lib/camera/browser-devices";
  import { getAudioDevices, type AudioDeviceInfo } from "$lib/ipc";
  import {
    COUNTDOWN_OPTIONS,
    countdownToken,
    type RecordingProfile,
  } from "$lib/profiles";
  import { profilesStore } from "$lib/stores/profiles.svelte";
  import {
    buildDuplicate,
    buildNewDraft,
    computeDialogWidth,
    DIALOG_ASIDE_W,
    DIALOG_MAIN_W,
    isCompactViewport,
    normalizeProfileForSave,
    summarize,
  } from "./profiles.logic";

  // mode = 'create' means draft is not yet in the store; mode = 'edit' means
  // draft mirrors an existing entry. Persistence only happens on Save.
  let mode = $state<"create" | "edit" | null>(null);
  let draft = $state<RecordingProfile | null>(null);
  let nameInputEl = $state<HTMLInputElement | null>(null);
  let query = $state("");

  // Refreshed each time the dialog opens since devices come and go between
  // recordings; camera enumeration may trigger a permission probe.
  let mics = $state<AudioDeviceInfo[]>([]);
  let cameras = $state<BrowserCamera[]>([]);
  let devicesLoading = $state(false);

  let viewportWidth = $state(
    typeof window !== "undefined" ? window.innerWidth : 1280,
  );
  $effect(() => {
    const onResize = () => (viewportWidth = window.innerWidth);
    onResize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });
  const isCompactDialog = $derived(isCompactViewport(viewportWidth));
  const showDevicePanel = $derived(
    !isCompactDialog && !!draft && (draft.microphone || draft.camera),
  );
  const dialogWidth = $derived(computeDialogWidth(viewportWidth, showDevicePanel));

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
    openDialog("create", buildNewDraft(profilesStore.profiles.length));
  }

  function duplicateProfile(profile: RecordingProfile) {
    openDialog("create", buildDuplicate(profile));
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
      toast.error("Name can't be empty");
      return;
    }
    const next = normalizeProfileForSave({ ...draft, name: trimmed });

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

  // Soft nudge: another saved profile with identical capture settings. Profiles
  // are told apart by name, so this informs without blocking the save.
  const twin = $derived.by(() =>
    draft ? profilesStore.twinOf(normalizeProfileForSave(draft)) : null,
  );

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

  // `null` = inherit the global countdown; `0` = off. Derived from the shared
  // COUNTDOWN_OPTIONS so the picker and the combination math can't drift.
  const countdownChoices: { value: number | null; label: string }[] =
    COUNTDOWN_OPTIONS.map((value) => ({
      value,
      label: value == null ? "Default" : value === 0 ? "Off" : `${value}s`,
    }));

  const countdownSegments: SegmentedOption<string>[] = countdownChoices.map(
    (c) => ({ value: countdownToken(c.value), label: c.label }),
  );

  function setDraftCountdown(value: number | null) {
    if (!draft) return;
    draft = { ...draft, countdown: value };
  }

  function setDraftCountdownToken(token: string) {
    setDraftCountdown(token === "inherit" ? null : Number(token));
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

  // Capture sources rendered as the faceplate readout at the bottom of each
  // card. On/off is carried by icon shape (Mic vs MicOff) as well as color, so
  // it doesn't depend on color alone.
  type Cap = {
    field: "systemAudio" | "microphone" | "camera";
    label: string;
    iconOn: typeof Volume2;
    iconOff: typeof Volume2;
  };
  const capabilities: Cap[] = [
    { field: "systemAudio", label: "System audio", iconOn: Volume2, iconOff: VolumeX },
    { field: "microphone", label: "Microphone", iconOn: Mic, iconOff: MicOff },
    { field: "camera", label: "Camera", iconOn: Camera, iconOff: VideoOff },
  ];
</script>

<div class="h-full overflow-y-auto scrollbar-transparent no-scrollbar">
  <div class="mx-auto flex max-w-5xl flex-col gap-8 px-6 py-10">
    <header class="flex flex-col gap-3">
      <span
        in:fly={{ y: 6, duration: 280, easing: cubicOut }}
        class="inline-flex w-fit items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.15em] text-muted-foreground/80 backdrop-blur transition-colors duration-200 hover:border-border hover:text-muted-foreground"
      >
        <SlidersIcon class="size-3 text-primary" />
        Profiles
      </span>
      <div
        in:fly={{ y: 12, duration: 320, delay: 40, easing: cubicOut }}
        class="flex items-end justify-between gap-3"
      >
        <h1
          class="text-balance text-[28px] font-semibold leading-tight tracking-tight text-foreground md:text-[32px]"
        >
          <span
            class="bg-linear-to-r from-foreground to-foreground/55 bg-clip-text text-transparent"
          >
            {profilesStore.profiles.length === 0
              ? "No profiles yet"
              : profilesStore.profiles.length === 1
                ? "1 profile"
                : `${profilesStore.profiles.length} profiles`}
          </span>
        </h1>
        <Button
          onclick={addProfile}
          size="sm"
          class="h-9 shrink-0 gap-1.5"
        >
          <Plus size={13} />
          New profile
          <Kbd class="bg-primary-foreground/15 text-primary-foreground/90"
            >⌘N</Kbd
          >
        </Button>
      </div>
      <p
        in:fly={{ y: 8, duration: 280, delay: 100, easing: cubicOut }}
        class="text-[12.5px] leading-relaxed text-muted-foreground"
      >
        Save what to capture (system audio, mic, camera) and pick the default
        that loads on launch.
      </p>
    </header>

    <!-- Profiles stay editable here but the recording panel won't auto-apply
         them until re-enabled. -->
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

    {#if filtered.length === 0}
      <div
        in:fade={{ duration: 200 }}
        class="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border/60 bg-card/40 p-12 text-center"
      >
        <div
          class="flex size-12 animate-empty-float items-center justify-center rounded-xl bg-foreground/5 text-muted-foreground ring-1 ring-inset ring-border/30"
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
              : "Create a profile to save what to capture."}
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
            role="button"
            tabindex="0"
            aria-label={`Edit ${profile.name}`}
            onclick={() => startEditing(profile)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                startEditing(profile);
              }
            }}
            class={cn(
              "group/card relative flex cursor-pointer flex-col overflow-hidden rounded-xl border shadow-(--shadow-craft-inset) outline-none transition-[background-color,border-color,box-shadow] duration-200 focus-visible:ring-2 focus-visible:ring-ring/60",
              profile.isDefault
                ? "border-primary/60 bg-card"
                : "border-border/40 bg-card hover:border-border hover:shadow-craft-sm",
            )}
          >
            <!-- Identity region, same treatment as a thumbnail-less recasts
                 card: muted surface, centered mark, and a `.recast`-style cutout
                 tab (here it carries the capability glyphs). -->
            <div class="relative h-24 shrink-0 overflow-hidden bg-muted/40">
              <div class="grid size-full place-items-center">
                <span
                  class={cn(
                    "flex size-12 items-center justify-center rounded-xl border text-[17px] font-semibold transition-colors",
                    profile.isDefault
                      ? "border-primary/30 bg-primary/10 text-primary"
                      : "border-border/50 bg-card text-muted-foreground group-hover/card:text-foreground",
                  )}
                >
                  {#if profile.isDefault}
                    <Star class="size-5" />
                  {:else}
                    {profile.name.trim().charAt(0).toUpperCase() || "?"}
                  {/if}
                </span>
              </div>

              {#if profile.isDefault}
                <span
                  class="absolute left-2 top-2 inline-flex items-center gap-1 rounded-md border border-primary/30 bg-primary/15 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-primary backdrop-blur-md"
                >
                  <Star size={9} /> Default
                </span>
              {/if}

              <Cutout
                corner="bl"
                surface="card"
                radius={8}
                class="flex items-center gap-1.5 px-2.5 pb-1 pt-2.5"
              >
                {#each capabilities as cap (cap.field)}
                  {@const on = profile[cap.field]}
                  {@const Icon = on ? cap.iconOn : cap.iconOff}
                  <Icon
                    class={cn(
                      "size-3 transition-colors",
                      on ? "text-primary" : "text-muted-foreground/40",
                    )}
                    aria-label={`${cap.label}: ${on ? "on" : "off"}`}
                  />
                {/each}
              </Cutout>
            </div>

            <!-- Info -->
            <div class="flex min-w-0 flex-1 flex-col gap-0.5 px-3 py-2.5">
              <div class="truncate text-[12.5px] font-semibold text-foreground">
                {profile.name}
              </div>
              <div class="truncate text-[10.5px] text-muted-foreground/80">
                {summarize(profile)}
              </div>
            </div>

            <!-- Actions, same placement/treatment as the recasts card. -->
            <div
              role="presentation"
              onclick={(e) => e.stopPropagation()}
              onkeydown={(e) => e.stopPropagation()}
              class="absolute right-2 top-2 z-20"
            >
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button
                      {...props as Record<string, unknown>}
                      variant="ghost"
                      size="icon-sm"
                      title="More actions"
                      class="size-7 rounded-lg border border-border/60 bg-background/80 text-foreground/60 opacity-0 backdrop-blur-md transition-all duration-200 hover:bg-background hover:text-foreground group-hover/card:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100"
                    >
                      <MoreHorizontal size={14} />
                    </Button>
                  {/snippet}
                </DropdownMenu.Trigger>
                <DropdownMenu.Content align="end" size="sm" class="w-44">
                  <DropdownMenu.Item onSelect={() => startEditing(profile)}>
                    <Pencil class="size-3" /> Edit profile
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onSelect={() => duplicateProfile(profile)}>
                    <Copy class="size-3" /> Duplicate
                  </DropdownMenu.Item>
                  {#if !profile.isDefault}
                    <DropdownMenu.Item onSelect={() => setDefault(profile.id)}>
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
          </div>
        {/each}

        <button
          type="button"
          onclick={addProfile}
          in:fly={{
            y: 8,
            duration: 240,
            delay: Math.min(filtered.length * 40, 280),
            easing: cubicOut,
          }}
          class="group/add flex h-full min-h-36 flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border/60 bg-card/30 p-6 text-center text-muted-foreground transition-all duration-200 hover:-translate-y-0.5 hover:border-primary/40 hover:bg-primary/5 hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
        >
          <span
            class="flex size-9 items-center justify-center rounded-lg bg-foreground/5 text-foreground transition-all duration-200 group-hover/add:scale-110 group-hover/add:bg-primary/10 group-hover/add:text-primary group-hover/add:shadow-[0_0_0_4px_color-mix(in_srgb,var(--color-primary)_12%,transparent)]"
          >
            <Plus class="size-4 transition-transform duration-300 group-hover/add:rotate-90" />
          </span>
          <div>
            <div class="text-[12.5px] font-semibold text-foreground">
              New profile
            </div>
            <div class="mt-0.5 text-[10.5px] text-muted-foreground/80">
              Save another capture setup
            </div>
          </div>
        </button>
      </div>
    {/if}
  </div>
</div>

{#snippet toggleRow(
  field: "isDefault" | "systemAudio" | "microphone" | "camera",
  Icon: typeof Star,
  label: string,
  hint: string,
)}
  <div class="flex items-center gap-3 px-5 py-3">
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
    <Switch
      checked={!!draft?.[field]}
      onCheckedChange={() => toggleDraft(field)}
      aria-label={label}
    />
  </div>
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

<!-- Factored so they render either inline (compact) or in the slide-out panel
     (wide) without duplicating the option mapping. -->
{#snippet micPicker()}
  {@render deviceRow(
    Mic,
    "Microphone device",
    "If unavailable at recording time, the system default is used.",
    mics.map((m) => ({
      value: m.id,
      label: m.name + (m.isDefault ? " (default)" : ""),
    })),
    draft?.micDeviceId ?? null,
    setMicSelection,
    "No microphones detected",
  )}
{/snippet}

{#snippet camPicker()}
  {@render deviceRow(
    Camera,
    "Camera device",
    "Saved by name; falls back to first non-virtual cam if missing.",
    cameras.map((c) => ({
      value: c.deviceId,
      label: c.label + (c.isVirtual ? " (virtual)" : ""),
    })),
    draft?.cameraDeviceId ?? null,
    setCameraSelection,
    "No cameras detected",
  )}
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
      style="width: {dialogWidth}px; max-width: calc(100vw - 2rem);"
      class="block! gap-0! overflow-hidden rounded-2xl p-0! ring-1 ring-border/60 shadow-(--shadow-craft-inset-strong) transition-[width] duration-300 ease-out"
    >
      <header
        class="flex items-center justify-between gap-3 border-b border-border/40 px-5 py-4"
      >
        <div class="min-w-0">
          <Dialog.Title
            class="text-[14px] font-semibold tracking-tight text-foreground"
          >
            {mode === "edit" ? "Edit profile" : "New profile"}
          </Dialog.Title>
          <Dialog.Description
            class="mt-0.5 text-[11px] font-medium text-muted-foreground"
          >
            Configure what to capture during recording.
          </Dialog.Description>
        </div>
        {#if draft.isDefault}
          <span
            class="inline-flex shrink-0 items-center gap-1 rounded-md border border-primary/30 bg-primary/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide text-primary"
          >
            <Star size={11} />
            Default
          </span>
        {/if}
      </header>

      <!-- Full-width so the form column and device panel start at the same Y. -->
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
          class="h-9 w-full rounded-lg border border-border/50 bg-input px-3 text-[13px] font-medium text-foreground outline-none transition-all placeholder:text-muted-foreground/60 focus:border-primary/60 focus:ring-2 focus:ring-primary/20"
        />
      </div>

      <div class="flex items-stretch">
        <!-- Fixed width on wide screens so it doesn't reflow when the device
             panel slides in; fluid on compact. -->
        <div
          class="flex min-w-0 flex-col divide-y divide-border/30"
          style={isCompactDialog
            ? "flex: 1 1 0; min-width: 0;"
            : `width: ${DIALOG_MAIN_W}px; flex: 0 0 ${DIALOG_MAIN_W}px;`}
        >
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
          {#if isCompactDialog && draft.microphone}
            {@render micPicker()}
          {/if}
          {@render toggleRow(
            "camera",
            Camera,
            "Camera",
            "Overlay webcam feed onto the recording",
          )}
          {#if isCompactDialog && draft.camera}
            {@render camPicker()}
          {/if}

          <!-- "Default" inherits the global countdown; the rest pin a
               per-profile value. -->
          <div class="flex items-center gap-3 px-5 py-3">
            <span
              class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background/70 text-muted-foreground ring-1 ring-inset ring-border/40"
              aria-hidden="true"
            >
              <Timer size={14} />
            </span>
            <span class="flex min-w-0 flex-1 flex-col gap-0.5">
              <span class="truncate text-[12.5px] font-semibold text-foreground">
                Countdown
              </span>
              <span
                class="truncate text-[11px] font-medium text-muted-foreground"
              >
                Seconds before capture starts.
              </span>
            </span>
            <Segmented
              options={countdownSegments}
              value={countdownToken(draft.countdown ?? null)}
              onValueChange={setDraftCountdownToken}
              fill={false}
              aria-label="Countdown before recording"
            />
          </div>
        </div>

        <!-- Slides out on wide screens when a device capability is on, keeping
             the form column short. Width transition morphs; fly adds the reveal. -->
        {#if showDevicePanel}
          <aside
            in:fly={{ x: 20, duration: 260, easing: cubicOut }}
            out:fly={{ x: 20, duration: 220, easing: cubicOut }}
            style="width: {DIALOG_ASIDE_W}px;"
            class="flex shrink-0 flex-col border-l border-border/40"
          >
            <div class="flex items-center gap-2 border-b border-border/30 px-5 py-3">
              <SlidersIcon size={12} class="text-muted-foreground" />
              <span
                class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground"
              >
                Devices
              </span>
            </div>
            <div class="flex flex-col divide-y divide-border/30">
              {#if draft.microphone}
                {@render micPicker()}
              {/if}
              {#if draft.camera}
                {@render camPicker()}
              {/if}
            </div>
          </aside>
        {/if}
      </div>

      {#if twin}
        <div
          class="flex items-center gap-2 border-t border-border/30 bg-muted/20 px-5 py-2.5 text-[11px] text-muted-foreground"
        >
          <Info class="size-3.5 shrink-0 text-muted-foreground/70" />
          <span>
            Same capture settings as
            <span class="font-semibold text-foreground">{twin.name}</span>.
          </span>
        </div>
      {/if}

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

<style>
  /* Gentle vertical float for empty-state iconography. */
  @keyframes empty-float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-3px);
    }
  }
  :global(.animate-empty-float) {
    animation: empty-float 4.2s cubic-bezier(0.45, 0, 0.55, 1) infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.animate-empty-float) {
      animation: none;
    }
  }
</style>
