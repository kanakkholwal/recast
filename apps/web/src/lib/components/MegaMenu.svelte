<script lang="ts">
import { ArrowRight, ArrowUpRight, ChevronDown } from "@recast/icons";
import { cn } from "@recast/ui/utils";
import { prefersReducedMotion } from "$lib/motion-core";
import type { MenuGroup } from "./nav-data";

/**
 * Desktop nav dropdowns sharing ONE panel that resizes and slides between
 * triggers, rather than a separate popover per item. Moving along the row
 * reads as the panel morphing to the next group, which is what makes the row
 * feel like one object instead of four.
 *
 * A disclosure pattern, not `role="menu"`: the contents are links to pages, so
 * the browser's own link semantics are what a screen reader should hear.
 */
let { groups, pathname }: { groups: MenuGroup[]; pathname: string } = $props();

const reduced = $derived(prefersReducedMotion());

let active = $state(-1);
const open = $derived(active >= 0);

// Measured from the active panel and the active trigger, so the container can
// animate between two known boxes instead of jumping.
let panels = $state<(HTMLElement | null)[]>([]);
let triggers = $state<(HTMLElement | null)[]>([]);
let row: HTMLElement | undefined = $state();
let box = $state({ width: 0, height: 0, left: 0 });

function measure() {
	if (active < 0 || !row) return;
	const panel = panels[active];
	const trigger = triggers[active];
	if (!panel || !trigger) return;
	const rowRect = row.getBoundingClientRect();
	const triggerRect = trigger.getBoundingClientRect();
	const width = panel.scrollWidth;
	// Centre the panel under its trigger, then keep it inside the row.
	const ideal = triggerRect.left - rowRect.left + triggerRect.width / 2 - width / 2;
	box = {
		width,
		height: panel.scrollHeight,
		left: Math.max(0, ideal),
	};
}

$effect(() => {
	// Re-measure whenever the active group changes.
	void active;
	measure();
});

// --- Hover intent -------------------------------------------------------
// A diagonal path from a trigger to the panel below leaves the row for a
// frame; closing instantly would make the menu unusable.
let closeTimer: ReturnType<typeof setTimeout> | null = null;
function cancelClose() {
	if (closeTimer) clearTimeout(closeTimer);
	closeTimer = null;
}
function scheduleClose() {
	cancelClose();
	closeTimer = setTimeout(() => (active = -1), 140);
}
function openGroup(i: number) {
	cancelClose();
	active = i;
}
function closeNow() {
	cancelClose();
	active = -1;
}

function onTriggerKeydown(e: KeyboardEvent, i: number) {
	if (e.key === "Enter" || e.key === " ") {
		e.preventDefault();
		active = active === i ? -1 : i;
	} else if (e.key === "ArrowDown") {
		e.preventDefault();
		openGroup(i);
		// Hand focus to the first link so the panel is keyboard-reachable.
		queueMicrotask(() => panels[i]?.querySelector<HTMLElement>("a")?.focus());
	} else if (e.key === "Escape") {
		closeNow();
		triggers[i]?.focus();
	}
}

const isCurrent = (href: string) => pathname === href || pathname.startsWith(`${href}/`);
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === "Escape") closeNow();
	}}
	onresize={measure}
/>

<!-- The row owns the hover region so moving between triggers never closes. -->
<div
	bind:this={row}
	class="relative hidden items-center gap-1 md:flex"
	onmouseleave={scheduleClose}
	onmouseenter={cancelClose}
	role="presentation"
>
	{#each groups as group, i (group.label)}
		{@const isOpen = active === i}
		<button
			bind:this={triggers[i]}
			type="button"
			aria-expanded={isOpen}
			aria-controls="megamenu-panel"
			onmouseenter={() => openGroup(i)}
			onfocus={() => openGroup(i)}
			onclick={() => (active = active === i ? -1 : i)}
			onkeydown={(e) => onTriggerKeydown(e, i)}
			class={cn(
				"inline-flex cursor-pointer items-center gap-1 whitespace-nowrap rounded-full px-3.5 py-2 text-sm font-medium transition-colors duration-200 outline-none focus-visible:ring-2 focus-visible:ring-primary motion-reduce:transition-none",
				isOpen || isCurrent(group.href)
					? "text-foreground"
					: "text-muted-foreground hover:text-foreground",
			)}
		>
			{group.label}
			<ChevronDown
				class={cn(
					"size-3.5 transition-transform duration-200 motion-reduce:transition-none",
					isOpen && "rotate-180",
				)}
			/>
		</button>
	{/each}

	<!-- One panel for every group. Size and offset animate; contents crossfade. -->
	<div
		id="megamenu-panel"
		aria-hidden={!open}
		class={cn(
			"absolute top-full z-50 overflow-hidden rounded-xl border border-border-low bg-card shadow-craft-floating",
			"origin-top",
			reduced
				? ""
				: "transition-[width,height,transform,opacity] duration-300 ease-[cubic-bezier(0.625,0.05,0,1)]",
			open ? "pointer-events-auto opacity-100" : "pointer-events-none opacity-0",
		)}
		style={`width:${box.width}px;height:${box.height}px;transform:translate3d(${box.left}px, ${open ? 8 : 2}px, 0) scale(${open ? 1 : 0.98});`}
		onmouseenter={cancelClose}
		onmouseleave={scheduleClose}
		role="presentation"
	>
		{#each groups as group, i (group.label)}
			{@const isOpen = active === i}
			<div
				bind:this={panels[i]}
				class={cn(
					"absolute inset-x-0 top-0 w-max",
					reduced ? "" : "transition-opacity duration-200 motion-reduce:transition-none",
					isOpen ? "opacity-100" : "pointer-events-none opacity-0",
				)}
				inert={!isOpen}
			>
				<ul class="grid w-[34rem] grid-cols-2 gap-1 p-2">
					{#each group.items as item (item.href)}
						{@const Icon = item.icon}
						<li>
							<a
								href={item.href}
								target={item.external ? "_blank" : undefined}
								rel={item.external ? "noopener noreferrer" : undefined}
								onclick={closeNow}
								aria-current={isCurrent(item.href) ? "page" : undefined}
								class="group/item flex gap-3 rounded-lg p-3 transition-colors hover:bg-paper aria-[current=page]:bg-paper motion-reduce:transition-none"
							>
								<Icon class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
								<span class="min-w-0">
									<span
										class="flex items-center gap-1 text-body-sm font-medium text-foreground"
									>
										{item.label}
										{#if item.external}
											<ArrowUpRight class="size-3 text-muted-foreground" />
										{/if}
									</span>
									<span class="mt-0.5 block text-caption text-muted-foreground">
										{item.description}
									</span>
								</span>
							</a>
						</li>
					{/each}
				</ul>

				{#if group.footer}
					<a
						href={group.footer.href}
						onclick={closeNow}
						class="group/cta flex items-center justify-between gap-4 border-t border-border-low bg-paper px-5 py-3 transition-colors hover:bg-background motion-reduce:transition-none"
					>
						<span class="text-body-sm font-medium text-foreground">{group.footer.label}</span>
						<span class="flex items-center gap-1.5 text-caption text-muted-foreground">
							{group.footer.hint}
							<ArrowRight
								class="size-3.5 transition-transform group-hover/cta:translate-x-0.5 motion-reduce:transition-none"
							/>
						</span>
					</a>
				{/if}
			</div>
		{/each}
	</div>
</div>
