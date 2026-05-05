<script lang="ts">
	import { mode, toggleMode } from "@recast/ui/theme";
	import { Moon, Sun } from "lucide-svelte";
</script>

<button
	type="button"
	onclick={toggleMode}
	aria-label="Toggle dark mode (dev)"
	title="Toggle theme · dev only"
	class="glass-chip group fixed bottom-5 right-5 z-[60] grid size-11 place-items-center rounded-full text-foreground/80 shadow-craft-floating transition-all hover:-translate-y-0.5 hover:text-foreground"
>
	{#if mode.current === "dark"}
		<Sun class="size-4 transition-transform group-hover:rotate-12" />
	{:else}
		<Moon class="size-4 transition-transform group-hover:-rotate-12" />
	{/if}
	<span
		class="pointer-events-none absolute -top-1 -right-1 rounded-full bg-primary px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-primary-foreground"
	>
		dev
	</span>
</button>
