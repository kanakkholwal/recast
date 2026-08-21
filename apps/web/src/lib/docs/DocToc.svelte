<script lang="ts">
import type { DocHeading } from "./headings";

interface Props {
	headings: DocHeading[];
	label?: string;
}

let { headings, label = "On this page" }: Props = $props();

let activeId = $state<string | null>(null);

// Highlight the section the reader is in. One observer over every heading,
// with a top margin that fires the swap as a heading clears the navbar.
$effect(() => {
	if (headings.length === 0 || typeof IntersectionObserver === "undefined") return;

	const targets = headings
		.map((heading) => document.getElementById(heading.id))
		.filter((element): element is HTMLElement => element !== null);
	if (targets.length === 0) return;

	const observer = new IntersectionObserver(
		(entries) => {
			const visible = entries.filter((entry) => entry.isIntersecting);
			if (visible.length > 0) activeId = visible[0].target.id;
		},
		{ rootMargin: "-96px 0px -70% 0px", threshold: 0 },
	);
	for (const target of targets) observer.observe(target);

	return () => observer.disconnect();
});
</script>

{#if headings.length > 1}
	<nav aria-label={label} class="lg:w-52 lg:shrink-0">
		<div class="lg:sticky lg:top-24">
			<p class="text-caption font-medium text-muted-foreground">{label}</p>
			<ul class="mt-2 flex flex-col gap-1.5 border-l border-border-low pl-3">
				{#each headings as heading (heading.id)}
					<li>
						<a
							href="#{heading.id}"
							aria-current={activeId === heading.id ? "true" : undefined}
							class="text-body-sm transition-colors hover:text-foreground"
							class:text-foreground={activeId === heading.id}
							class:font-medium={activeId === heading.id}
							class:text-muted-foreground={activeId !== heading.id}
						>
							{heading.text}
						</a>
					</li>
				{/each}
			</ul>
		</div>
	</nav>
{/if}
