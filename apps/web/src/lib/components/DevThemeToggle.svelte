<script lang="ts">
import { Moon, Sun } from "@recast/icons";
import { mode, toggleMode } from "@recast/ui/theme";
</script>

<button
	type="button"
	onclick={toggleMode}
	aria-label="Toggle dark mode (dev)"
	title="Toggle theme · dev only · ⌘/Ctrl + Shift + L"
	class="group fixed bottom-20 right-5 z-[60] grid size-11 cursor-pointer place-items-center rounded-full border border-border-low bg-card text-muted-foreground shadow-craft-floating transition-colors hover:text-foreground motion-reduce:transition-none"
>
	{#if mode.current === "dark"}
		<Sun class="size-4 transition-transform group-hover:rotate-12" />
	{:else}
		<Moon class="size-4 transition-transform group-hover:-rotate-12" />
	{/if}
	<span
		class="pointer-events-none absolute -top-1 -right-1 rounded-full bg-primary px-1.5 text-caption font-medium text-primary-foreground"
	>
		dev
	</span>
</button>
