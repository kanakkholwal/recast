<script lang="ts">
	import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
	import { recastsStore, settingsStore } from "$lib/dashboard/store.svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import { RotateCcw, Settings2 } from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	const prefs = settingsStore.value.preferences;

	function savePreferences() {
		settingsStore.save();
		toast.success("Preferences saved.");
	}

	function resetData() {
		recastsStore.reset();
		toast.success("Sample recasts restored.");
	}
</script>

<div class="flex flex-col gap-4">
	<div in:fly={{ y: 14, duration: 420, easing: cubicOut }}>
		<SettingsSection
			icon={Settings2}
			title="Preferences"
			description="Defaults applied to new recasts."
		>
			<div class="flex flex-col gap-4">
				<label class="flex items-center justify-between gap-4">
					<span class="text-sm">
						<span class="font-medium text-foreground">Default destination</span>
						<span class="block text-xs text-muted-foreground">
							Where uploaded recasts land.
						</span>
					</span>
					<select
						bind:value={prefs.defaultDestination}
						onchange={savePreferences}
						class="rounded-lg border border-border-low/70 bg-background/80 px-3 py-2 text-sm font-medium text-foreground outline-none transition-colors focus:border-primary/60"
					>
						<option value="local">Local</option>
						<option value="cloud">Cloud (Cloudinary)</option>
					</select>
				</label>

				<label class="flex cursor-pointer items-center justify-between gap-4">
					<span class="text-sm">
						<span class="font-medium text-foreground">Auto-upload to cloud</span>
						<span class="block text-xs text-muted-foreground">
							Push new recasts to Cloudinary automatically.
						</span>
					</span>
					<input
						type="checkbox"
						bind:checked={prefs.autoUpload}
						onchange={savePreferences}
						class="size-4 accent-primary"
					/>
				</label>
			</div>
		</SettingsSection>
	</div>

	<div
		class="flex items-center justify-between gap-4 rounded-xl border border-dashed border-border-low/70 p-5"
		in:fly={{ y: 14, duration: 420, delay: 80, easing: cubicOut }}
	>
		<div>
			<h2 class="text-sm font-semibold text-foreground">Sample data</h2>
			<p class="mt-0.5 text-xs text-muted-foreground">
				Restore the default recasts library on this device.
			</p>
		</div>
		<Button variant="outline" size="sm" class="gap-2" onclick={resetData}>
			<RotateCcw class="size-3.5" />
			Reset
		</Button>
	</div>
</div>
