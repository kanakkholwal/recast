<script lang="ts">
	import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
	import { settingsStore } from "$lib/dashboard/store.svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import { User } from "lucide-svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	const settings = settingsStore.value;

	const inputClass =
		"rounded-lg border border-border-low/70 bg-background/80 px-3 py-2 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/60 focus:border-primary/60";

	function save(e: SubmitEvent) {
		e.preventDefault();
		settingsStore.save();
		toast.success("Profile updated.");
	}
</script>

<div in:fly={{ y: 14, duration: 420, easing: cubicOut }}>
	<SettingsSection
		icon={User}
		title="Profile"
		description="How you show up across Recast."
	>
		<form class="grid gap-4 sm:grid-cols-2" onsubmit={save}>
			<label class="flex flex-col gap-1.5">
				<span class="text-xs font-semibold text-foreground/85">Display name</span>
				<input type="text" bind:value={settings.profile.name} class={inputClass} />
			</label>
			<label class="flex flex-col gap-1.5">
				<span class="text-xs font-semibold text-foreground/85">Email</span>
				<input type="email" bind:value={settings.profile.email} class={inputClass} />
			</label>
			<div class="sm:col-span-2">
				<Button type="submit" variant="outline" size="sm">Save changes</Button>
			</div>
		</form>
	</SettingsSection>
</div>
