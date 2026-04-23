<script lang="ts">
  import { RecastList, type RecastAccessory, type RecastListItem } from "$components/recast";
  import {
    Camera,
    CircleCheck,
    Copy,
    Mic,
    Pencil,
    Plus,
    SlidersHorizontal as SlidersIcon,
    Star,
    Trash2,
    Volume2,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Dialog from "@recast/ui/dialog";
  import { toast } from "@recast/ui/sonner";
  import { cn } from "@recast/ui/utils";
  import { onMount } from "svelte";

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
        { id: crypto.randomUUID(), name: "Screen Only", systemAudio: true, microphone: false, camera: false, isDefault: true },
        { id: crypto.randomUUID(), name: "Presentation", systemAudio: true, microphone: true, camera: true, isDefault: false },
        { id: crypto.randomUUID(), name: "Tutorial", systemAudio: true, microphone: true, camera: false, isDefault: false },
      ];
    }
    profiles = ensureExactlyOneDefault(profiles);
    save();

    window.addEventListener("keydown", handleGlobalShortcut);
    return () => window.removeEventListener("keydown", handleGlobalShortcut);
  });

  function ensureExactlyOneDefault(list: RecordingProfile[]): RecordingProfile[] {
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
    toast.success(`Duplicated “${profile.name}”`);
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
    toast.success(`Deleted “${victim.name}”`);
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

  function toggleOption(id: string, option: "systemAudio" | "microphone" | "camera") {
    profiles = profiles.map((p) =>
      p.id === id ? { ...p, [option]: !p[option] } : p,
    );
    save();
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

  function toggleDraft(field: "systemAudio" | "microphone" | "camera" | "isDefault") {
    if (!draft) return;
    if (field === "isDefault" && draft.isDefault) {
      const otherCandidates = profiles.filter((p) => p.id !== draft!.id);
      if (otherCandidates.length === 0) {
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

  function buildAccessories(profile: RecordingProfile): RecastAccessory[] {
    const accs: RecastAccessory[] = [];
    if (profile.isDefault) accs.push({ icon: Star, text: "Default", variant: "warning" });
    if (profile.systemAudio) accs.push({ icon: Volume2, tooltip: "System audio on", variant: "info" });
    if (profile.microphone) accs.push({ icon: Mic, tooltip: "Microphone on", variant: "success" });
    if (profile.camera) accs.push({ icon: Camera, tooltip: "Camera on", variant: "default" });
    return accs;
  }

  const items = $derived<RecastListItem[]>(
    profiles.map((profile) => ({
      id: profile.id,
      title: profile.name,
      subtitle: [
        profile.systemAudio && "System audio",
        profile.microphone && "Mic",
        profile.camera && "Camera",
      ].filter(Boolean).join(" · ") || "Silent",
      icon: profile.isDefault ? Star : SlidersIcon,
      iconClass: profile.isDefault ? "text-warning" : undefined,
      keywords: [profile.name, profile.isDefault ? "default" : ""],
      accessories: buildAccessories(profile),
      layout: "row",
      onSelect: () => startEditing(profile),
      actions: [
        {
          id: "edit",
          label: "Edit Profile…",
          icon: Pencil,
          shortcut: "⌘R",
          onAction: () => startEditing(profile),
        },
        {
          id: "duplicate",
          label: "Duplicate",
          icon: Copy,
          shortcut: "⌘D",
          onAction: () => duplicateProfile(profile),
        },
        {
          id: "set-default",
          label: profile.isDefault ? "Already Default" : "Set as Default",
          icon: CircleCheck,
          onAction: () => {
            if (!profile.isDefault) setDefault(profile.id);
          },
        },
        {
          id: "toggle-audio",
          label: profile.systemAudio ? "Disable System Audio" : "Enable System Audio",
          icon: Volume2,
          onAction: () => toggleOption(profile.id, "systemAudio"),
        },
        {
          id: "toggle-mic",
          label: profile.microphone ? "Disable Microphone" : "Enable Microphone",
          icon: Mic,
          onAction: () => toggleOption(profile.id, "microphone"),
        },
        {
          id: "toggle-camera",
          label: profile.camera ? "Disable Camera" : "Enable Camera",
          icon: Camera,
          onAction: () => toggleOption(profile.id, "camera"),
        },
        {
          id: "delete",
          label: "Delete Profile",
          icon: Trash2,
          variant: "destructive",
          shortcut: "⌘⌫",
          onAction: () => deleteProfile(profile.id),
        },
      ],
    })),
  );

  function handleDialogKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      finishEditing();
    }
  }
</script>

<RecastList
  {items}
  isLoading={false}
  title="Recording Profiles"
  subtitle="Save different recording configurations for quick access"
  searchPlaceholder="Search profiles..."
  emptyTitle="No profiles yet"
  emptyHint="Create a profile to save your recording presets."
>
  {#snippet toolbar()}
    <Button variant="ghost" size="icon-sm" onclick={addProfile} title="New Profile (⌘N)">
      <Plus size={14} />
    </Button>
  {/snippet}
</RecastList>

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
      class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background/70 ring-1 ring-inset ring-border/40 text-muted-foreground"
    >
      <Icon size={14} />
    </span>
    <span class="flex min-w-0 flex-1 flex-col gap-0.5">
      <span class="truncate text-[12.5px] font-semibold text-foreground">{label}</span>
      <span class="truncate text-[11px] font-medium text-muted-foreground">{hint}</span>
    </span>
    <span
      class={cn(
        "flex h-5 w-9 shrink-0 items-center rounded-full transition-colors",
        draft?.[field] ? "bg-primary" : "bg-muted ring-1 ring-inset ring-border/50",
      )}
    >
      <span
        class={cn(
          "size-4 rounded-full bg-card shadow-sm transition-transform",
          draft?.[field] ? "translate-x-[1.125rem]" : "translate-x-0.5",
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
          <Dialog.Title class="text-[14px] font-semibold tracking-tight text-foreground">
            {editingId && profiles.find((p) => p.id === editingId) ? "Edit Profile" : "New Profile"}
          </Dialog.Title>
          <Dialog.Description class="mt-0.5 text-[11px] font-medium text-muted-foreground">
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
        {@render toggleRow("isDefault", Star, "Default profile", "Use this profile automatically on launch")}
        {@render toggleRow("systemAudio", Volume2, "System audio", "Capture sounds playing on your device")}
        {@render toggleRow("microphone", Mic, "Microphone", "Record your voice from the default input")}
        {@render toggleRow("camera", Camera, "Camera", "Overlay webcam feed onto the recording")}
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
          <Button variant="ghost" size="xs" onclick={cancelEditing}>Cancel</Button>
          <Button variant="default" size="xs" class="gap-2" onclick={finishEditing}>
            Save
            <kbd
              class="rounded border border-primary-foreground/20 bg-primary-foreground/10 px-1 py-0.5 font-mono text-[9px] font-semibold"
            >
              ⌘↵
            </kbd>
          </Button>
        </div>
      </footer>
    </Dialog.Content>
  </Dialog.Root>
{/if}
