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
  import { toast } from "@recast/ui/sonner";
  import { cn } from "@recast/ui/utils";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  interface RecordingProfile {
    id: string;
    name: string;
    systemAudio: boolean;
    microphone: boolean;
    camera: boolean;
    isDefault: boolean;
  }

  const STORAGE_KEY = "recast-recording-profiles";

  let profiles = $state<RecordingProfile[]>([]);
  let editingId = $state<string | null>(null);
  let draft = $state<RecordingProfile | null>(null);
  let nameInputEl = $state<HTMLInputElement | null>(null);
  let query = $state("");

  onMount(() => {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      try {
        profiles = JSON.parse(stored);
      } catch {
        profiles = [];
      }
    }
    if (profiles.length === 0) {
      profiles = [
        {
          id: crypto.randomUUID(),
          name: "Screen Only",
          systemAudio: true,
          microphone: false,
          camera: false,
          isDefault: true,
        },
        {
          id: crypto.randomUUID(),
          name: "Presentation",
          systemAudio: true,
          microphone: true,
          camera: true,
          isDefault: false,
        },
        {
          id: crypto.randomUUID(),
          name: "Tutorial",
          systemAudio: true,
          microphone: true,
          camera: false,
          isDefault: false,
        },
      ];
    }
    profiles = ensureExactlyOneDefault(profiles);
    save();

    window.addEventListener("keydown", handleGlobalShortcut);
    return () => window.removeEventListener("keydown", handleGlobalShortcut);
  });

  function ensureExactlyOneDefault(
    list: RecordingProfile[],
  ): RecordingProfile[] {
    if (list.length === 0) return list;
    const defaults = list.filter((p) => p.isDefault);
    if (defaults.length === 1) return list;
    if (defaults.length === 0) {
      return list.map((p, i) => (i === 0 ? { ...p, isDefault: true } : p));
    }
    let seen = false;
    return list.map((p) => {
      if (p.isDefault && !seen) {
        seen = true;
        return p;
      }
      return p.isDefault ? { ...p, isDefault: false } : p;
    });
  }

  function save() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(profiles));
  }

  function addProfile() {
    const profile: RecordingProfile = {
      id: crypto.randomUUID(),
      name: `Profile ${profiles.length + 1}`,
      systemAudio: true,
      microphone: false,
      camera: false,
      isDefault: profiles.length === 0,
    };
    profiles = [...profiles, profile];
    save();
    startEditing(profile);
  }

  function duplicateProfile(profile: RecordingProfile) {
    const copy: RecordingProfile = {
      ...profile,
      id: crypto.randomUUID(),
      name: `${profile.name} Copy`,
      isDefault: false,
    };
    profiles = [...profiles, copy];
    save();
    toast.success(`Duplicated "${profile.name}"`);
  }

  function deleteProfile(id: string) {
    const victim = profiles.find((p) => p.id === id);
    if (!victim) return;
    const wasDefault = victim.isDefault;
    profiles = profiles.filter((p) => p.id !== id);
    if (wasDefault && profiles.length > 0) {
      profiles = ensureExactlyOneDefault(profiles);
    }
    save();
    toast.success(`Deleted "${victim.name}"`);
    if (editingId === id) {
      editingId = null;
      draft = null;
    }
  }

  function setDefault(id: string) {
    profiles = profiles.map((p) => ({ ...p, isDefault: p.id === id }));
    save();
    toast.success("Default profile updated");
  }

  function startEditing(profile: RecordingProfile) {
    editingId = profile.id;
    draft = { ...profile };
    queueMicrotask(() => {
      nameInputEl?.focus();
      nameInputEl?.select();
    });
  }

  function finishEditing() {
    if (!editingId || !draft) return;
    if (!draft.name.trim()) {
      toast.error("Name cannot be empty");
      return;
    }
    const next = { ...draft, name: draft.name.trim() };
    const currentId = editingId;
    if (next.isDefault) {
      profiles = profiles.map((p) => ({
        ...(p.id === currentId ? next : p),
        isDefault: p.id === currentId,
      }));
    } else {
      profiles = profiles.map((p) => (p.id === currentId ? next : p));
      profiles = ensureExactlyOneDefault(profiles);
    }
    save();
    toast.success("Profile saved");
    editingId = null;
    draft = null;
  }

  function cancelEditing() {
    editingId = null;
    draft = null;
  }

  function toggleDraft(
    field: "systemAudio" | "microphone" | "camera" | "isDefault",
  ) {
    if (!draft) return;
    if (field === "isDefault" && draft.isDefault) {
      const others = profiles.filter((p) => p.id !== draft!.id);
      if (others.length === 0) {
        toast.info("At least one profile must be default");
        return;
      }
    }
    draft = { ...draft, [field]: !draft[field] };
  }

  function handleGlobalShortcut(e: KeyboardEvent) {
    const meta = e.metaKey || e.ctrlKey;
    if (!meta || e.shiftKey || e.altKey) return;
    if (editingId) return;
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

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return profiles;
    return profiles.filter((p) => p.name.toLowerCase().includes(q));
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
      profile.microphone && "Mic",
      profile.camera && "Camera",
    ].filter(Boolean) as string[];
    return parts.length === 0 ? "Silent capture" : parts.join(" · ");
  }
</script>

<div class="h-full overflow-y-auto scrollbar-transparent">
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
            {profiles.length === 0
              ? "No profiles yet"
              : profiles.length === 1
                ? "1 recording preset"
                : `${profiles.length} recording presets`}
          </span>
        </h1>
        <Button
          onclick={addProfile}
          size="sm"
          class="h-9 shrink-0 gap-1.5"
          title="New profile (⌘N)"
        >
          <Plus size={13} />
          New profile
          <Kbd class="bg-primary-foreground/15 text-primary-foreground/90"
            >⌘N</Kbd
          >
        </Button>
      </div>
      <p class="text-[12.5px] leading-relaxed text-muted-foreground">
        Save what to capture — system audio, mic, camera — and pick the default
        that loads on launch.
      </p>
    </header>

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
                  <DropdownMenu.Item onSelect={() => duplicateProfile(profile)}>
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

{#if editingId !== null && draft}
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
            {editingId && profiles.find((p) => p.id === editingId)
              ? "Edit Profile"
              : "New Profile"}
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
        {@render toggleRow(
          "camera",
          Camera,
          "Camera",
          "Overlay webcam feed onto the recording",
        )}
      </div>

      <footer
        class="flex items-center justify-between gap-2 border-t border-border/40 bg-muted/30 px-3 py-2.5"
      >
        <Button
          variant="destructive_soft"
          size="xs"
          class="gap-1.5"
          onclick={() => {
            if (editingId) deleteProfile(editingId);
          }}
        >
          <Trash2 size={12} />
          Delete
        </Button>
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
