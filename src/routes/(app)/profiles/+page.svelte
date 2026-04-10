<script lang="ts">
  import { RaycastList, type RaycastAccessory, type RaycastListItem } from "$components/raycast";
  import { Button } from "$components/ui/button";
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
  let editingName = $state("");

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
      save();
    }
  });

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
      isDefault: false,
    };
    profiles = [...profiles, profile];
    save();
    startEditing(profile.id, profile.name);
  }

  function deleteProfile(id: string) {
    if (profiles.length <= 1) {
      toast.error("Keep at least one profile");
      return;
    }
    const wasDefault = profiles.find((p) => p.id === id)?.isDefault;
    profiles = profiles.filter((p) => p.id !== id);
    if (wasDefault && profiles.length > 0) {
      profiles[0].isDefault = true;
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

  function startEditing(id: string, name: string) {
    editingId = id;
    editingName = name;
    setTimeout(() => {
      const input = document.getElementById("profile-rename-input") as HTMLInputElement | null;
      input?.focus();
      input?.select();
    }, 50);
  }

  function finishEditing() {
    if (editingId && editingName.trim()) {
      profiles = profiles.map((p) =>
        p.id === editingId ? { ...p, name: editingName.trim() } : p,
      );
      save();
    }
    editingId = null;
    editingName = "";
  }

  function cancelEditing() {
    editingId = null;
    editingName = "";
  }

  function buildAccessories(profile: RecordingProfile): RaycastAccessory[] {
    const accs: RaycastAccessory[] = [];
    if (profile.isDefault) accs.push({ icon: Star, text: "Default", variant: "warning" });
    if (profile.systemAudio) accs.push({ icon: Volume2, tooltip: "System audio on", variant: "info" });
    if (profile.microphone) accs.push({ icon: Mic, tooltip: "Microphone on", variant: "success" });
    if (profile.camera) accs.push({ icon: Camera, tooltip: "Camera on", variant: "default" });
    return accs;
  }

  const items = $derived<RaycastListItem[]>(
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
      actions: [
        {
          id: "rename",
          label: "Rename Profile",
          icon: Pencil,
          onAction: () => startEditing(profile.id, profile.name),
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

<RaycastList
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
</RaycastList>

{#if editingId !== null}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center bg-background/60 pt-32 backdrop-blur-sm"
    role="presentation"
    onclick={cancelEditing}
    onkeydown={(e) => e.key === "Escape" && cancelEditing()}
  >
    <div
      role="dialog"
      tabindex="-1"
      aria-label="Rename profile"
      class="w-full max-w-sm rounded-xl border border-border bg-popover p-4 shadow-2xl"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <label for="profile-rename-input" class="text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
        >Rename profile</label
      >
      <input
        id="profile-rename-input"
        bind:value={editingName}
        onkeydown={(e) => {
          if (e.key === "Enter") finishEditing();
          if (e.key === "Escape") cancelEditing();
        }}
        class="mt-2 w-full rounded-md border border-input bg-input/30 px-3 py-2 text-sm text-foreground outline-none focus:border-primary"
      />
      <div class="mt-3 flex items-center justify-end gap-2">
        <Button variant="ghost" size="sm" onclick={cancelEditing}>Cancel</Button>
        <Button variant="default" size="sm" onclick={finishEditing}>Save</Button>
      </div>
    </div>
  </div>
{/if}
