<script lang="ts">
	import Logo from "$lib/logo.svelte";
	import { FloatingMenu } from "$lib/motion-core";

	const menuGroups = [
		{
			title: "Product",
			variant: "default" as const,
			links: [
				{ label: "Features", href: "/features" },
				{ label: "Workflow", href: "/#workflow" },
				{ label: "Use cases", href: "/#use-cases" },
			],
		},
		{
			title: "Resources",
			variant: "muted" as const,
			links: [
				{ label: "Changelog", href: "/changelog" },
				{ label: "GitHub", href: "https://github.com/kanakkholwal/recast" },
				{ label: "Contact", href: "mailto:recast@nexonauts.com" },
			],
		},
	];

	let menuContainer: HTMLElement;
</script>

<div
	bind:this={menuContainer}
	class="fixed inset-x-0 top-4 z-50 flex justify-center px-4"
>
	<FloatingMenu
		portalTarget={menuContainer}
		{menuGroups}
		primaryButton={{ label: "Download", href: "/download" }}
		secondaryButton={{
			label: "GitHub",
			href: "https://github.com/kanakkholwal/recast",
		}}
		classes={{
			root: "shadow-craft-floating ring-1 ring-border-low",
			header: "px-4 py-2",
		}}
	>
		{#snippet logo()}
			<a
				href="/"
				class="group/logo flex items-center gap-2.5 transition-transform active:scale-[0.97]"
				aria-label="Recast — home"
			>
				<span
					class="grid size-7 place-items-center rounded-lg bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
				>
					<Logo size="20" color="transparent" fill="currentColor" />
				</span>
				<span class="text-[15px] font-semibold tracking-tight text-foreground">
					Recast
				</span>
			</a>
		{/snippet}
	</FloatingMenu>
</div>
