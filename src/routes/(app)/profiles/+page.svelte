<script lang="ts">
  import { RecastList, type RecastAccessory, type RecastListItem } from "$components/recast";
  import { Button } from "$components/ui/button";
  import { cn } from "$lib/utils";
  import {
    Camera,
    CheckCircle2,
    Mic,
    Pencil,
    Plus,
    SlidersHorizontal as SlidersIcon,
    Star,
    Trash2,
    Volume2,
  } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";

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
    // Edge case: exactly one profile must be marked default. Repair loaded state.
    profiles = ensureExactlyOneDefault(profiles);
    save();
  });

  /** Guarantees exactly one profile is marked default (when any profiles exist). */
  function ensureExactlyOneDefault(list: RecordingProfile[]): RecordingProfile[] {
    if (list.length === 0) return list;
    const defaults = list.filter((p) => p.isDefault);
    if (defaults.length === 1) return list;
    if (defaults.length === 0) {
      // None default → promote the first one.
      return list.map((p, i) => (i === 0 ? { ...p, isDefault: true } : p));
    }
    // Multiple defaults → keep the first, clear the rest.
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
      // First profile auto-becomes default.
      isDefault: profiles.length === 0,
    };
    profiles = [...profiles, profile];
    save();
    startEditing(profile);
  }

  function deleteProfile(id: string) {
    if (profiles.length <= 1) {
      toast.error("Keep at least one profile");
      return;
    }
    const wasDefault = profiles.find((p) => p.id === id)?.isDefault;
    profiles = profiles.filter((p) => p.id !== id);
    // If we removed the default, promote the first remaining profile.
    if (wasDefault) {
      profiles = ensureExactlyOneDefault(profiles);
    }
    save();
    toast.success("Profile deleted");
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
    setTimeout(() => {
      const input = document.getElementById("profile-name-input") as HTMLInputElement | null;
      input?.focus();
      input?.select();
    }, 50);
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
      // Applying default to this one unsets all others.
      profiles = profiles.map((p) => ({
        ...(p.id === currentId ? next : p),
        isDefault: p.id === currentId,
      }));
    } else {
      profiles = profiles.map((p) => (p.id === currentId ? next : p));
      // Edge case: user unchecked default on the only default → keep this one default.
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
    // Edge case: don't allow unchecking "default" if this profile is the only default
    // AND no other profile exists to take over.
    if (field === "isDefault" && draft.isDefault) {
      const otherCandidates = profiles.filter((p) => p.id !== draft!.id);
      if (otherCandidates.length === 0) {
        toast.info("At least one profile must be default");
        return;
      }
    }
    draft = { ...draft, [field]: !draft[field] };
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
      onSelect: () => startEditing(profile),
      actions: [
        {
          id: "edit",
          label: "Edit Profile…",
          icon: Pencil,
          onAction: () => startEditing(profile),
        },
        {
          id: "set-default",
          label: profile.isDefault ? "Already Default" : "Set as Default",
          icon: CheckCircle2,
          onAction: () => !profile.isDefault && setDefault(profile.id),
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
    <Button variant="ghost" size="icon-sm" onclick={addProfile} title="New Profile">
      <Plus size={14} />
    </Button>
  {/snippet}
</RecastList>

{#if editingId !== null && draft}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center bg-background/60 pt-24 backdrop-blur-sm"
    role="presentation"
    onclick={cancelEditing}
    onkeydown={(e) => e.key === "Escape" && cancelEditing()}
  >
    <div
      role="dialog"
      tabindex="-1"
      aria-label="Edit profile"
      class="w-full max-w-md overflow-hidden rounded-xl border border-border bg-popover shadow-2xl ring-1 ring-border"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") cancelEditing();
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) finishEditing();
      }}
    >
      <header class="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
        <div class="min-w-0">
          <h3 class="truncate text-[13px] font-semibold tracking-tight text-foreground">Edit Profile</h3>
          <p class="truncate text-[11px] text-muted-foreground">Configure what to capture</p>
        </div>
        {#if draft.isDefault}
          <span
            class="inline-flex shrink-0 items-center gap-1 rounded border border-warning/20 bg-warning/10 px-1.5 py-0.5 text-[10px] font-medium text-warning"
          >
            <Star size={11} />
            Default
          </span>
        {/if}
      </header>

      <div class="flex flex-col">
        <!-- Name -->
        <div class="flex items-center gap-4 border-b border-border px-4 py-3">
          <label for="profile-name-input" class="w-28 shrink-0 text-[12px] font-medium text-foreground"
            >Name</label
          >
          <input
            id="profile-name-input"
            bind:value={draft.name}
            class="h-8 flex-1 rounded-md border border-input bg-input/30 px-2.5 text-[12px] text-foreground outline-none focus:border-primary"
          />
        </div>

        <!-- Default toggle -->
        <button
          type="button"
          onclick={() => toggleDraft("isDefault")}
          class="flex items-center gap-4 border-b border-border px-4 py-3 text-left transition-colors hover:bg-muted/40"
        >
          <span class="flex w-28 shrink-0 items-center gap-2 text-[12px] font-medium text-foreground">
            <Star size={14} class="text-muted-foreground" />
            Default
          </span>
          <span class="flex-1 truncate text-[11px] text-muted-foreground">
            Use this profile automatically on launch
          </span>
          <span
            class={cn(
              "flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors",
              draft.isDefault ? "border-primary bg-primary" : "border-border bg-muted"
            )}
          >
            <span
              class={cn(
                "size-4 rounded-full bg-background shadow transition-transform",
                draft.isDefault ? "translate-x-4" : "translate-x-0.5"
              )}
            ></span>
          </span>
        </button>

        <!-- System audio toggle -->
        <button
          type="button"
          onclick={() => toggleDraft("systemAudio")}
          class="flex items-center gap-4 border-b border-border px-4 py-3 text-left transition-colors hover:bg-muted/40"
        >
          <span class="flex w-28 shrink-0 items-center gap-2 text-[12px] font-medium text-foreground">
            <Volume2 size={14} class="text-muted-foreground" />
            System Audio
          </span>
          <span class="flex-1 truncate text-[11px] text-muted-foreground">
            Capture sounds playing on your device
          </span>
          <span
            class={cn(
              "flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors",
              draft.systemAudio ? "border-primary bg-primary" : "border-border bg-muted"
            )}
          >
            <span
              class={cn(
                "size-4 rounded-full bg-background shadow transition-transform",
                draft.systemAudio ? "translate-x-4" : "translate-x-0.5"
              )}
            ></span>
          </span>
        </button>

        <!-- Microphone toggle -->
        <button
          type="button"
          onclick={() => toggleDraft("microphone")}
          class="flex items-center gap-4 border-b border-border px-4 py-3 text-left transition-colors hover:bg-muted/40"
        >
          <span class="flex w-28 shrink-0 items-center gap-2 text-[12px] font-medium text-foreground">
            <Mic size={14} class="text-muted-foreground" />
            Microphone
          </span>
          <span class="flex-1 truncate text-[11px] text-muted-foreground">
            Record your voice from the default input
          </span>
          <span
            class={cn(
              "flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors",
              draft.microphone ? "border-primary bg-primary" : "border-border bg-muted"
            )}
          >
            <span
              class={cn(
                "size-4 rounded-full bg-background shadow transition-transform",
                draft.microphone ? "translate-x-4" : "translate-x-0.5"
              )}
            ></span>
          </span>
        </button>

        <!-- Camera toggle -->
        <button
          type="button"
          onclick={() => toggleDraft("camera")}
          class="flex items-center gap-4 border-b border-border px-4 py-3 text-left transition-colors hover:bg-muted/40"
        >
          <span class="flex w-28 shrink-0 items-center gap-2 text-[12px] font-medium text-foreground">
            <Camera size={14} class="text-muted-foreground" />
            Camera
          </span>
          <span class="flex-1 truncate text-[11px] text-muted-foreground">
            Overlay webcam feed onto the recording
          </span>
          <span
            class={cn(
              "flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors",
              draft.camera ? "border-primary bg-primary" : "border-border bg-muted"
            )}
          >
            <span
              class={cn(
                "size-4 rounded-full bg-background shadow transition-transform",
                draft.camera ? "translate-x-4" : "translate-x-0.5"
              )}
            ></span>
          </span>
        </button>
      </div>

      <footer
        class="flex h-10 items-center justify-between border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
      >
        <div class="flex items-center gap-3">
          <span class="flex items-center gap-1">
            <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">⌘↵</kbd>
            <span>Save</span>
          </span>
          <span class="flex items-center gap-1">
            <kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">esc</kbd>
            <span>Cancel</span>
          </span>
        </div>
        <div class="flex items-center gap-1.5">
          <Button variant="ghost" size="sm" class="h-7" onclick={cancelEditing}>Cancel</Button>
          <Button variant="default" size="sm" class="h-7" onclick={finishEditing}>Save</Button>
        </div>
      </footer>
    </div>
  </div>
{/if}
