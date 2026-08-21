<script lang="ts">
import { Plus } from "@recast/icons";
import * as Collapsible from "@recast/ui/collapsible";

// One open at a time. Collapsible animates the real height, which `<details>`
// cannot do.
let { items }: { items: Array<{ q: string; a: string }> } = $props();

// First row opens on load so the affordance reads without a click.
let openIndex = $state(0);
</script>

<ul class="divide-y divide-border-low border-y border-border-low">
	{#each items as item, i (item.q)}
		<li>
			<Collapsible.Root
				bind:open={() => openIndex === i, (v) => (openIndex = v ? i : -1)}
				class="group/faq"
			>
				<Collapsible.Trigger
					class="flex w-full cursor-pointer items-center justify-between gap-6 py-5 text-left text-body font-medium text-foreground"
				>
					{item.q}
					<Plus
						class="size-4 shrink-0 text-muted-foreground transition-transform duration-300 ease-[cubic-bezier(0.625,0.05,0,1)] group-data-[state=open]/faq:rotate-45 motion-reduce:transition-none"
					/>
				</Collapsible.Trigger>
				<Collapsible.Content>
					<p class="max-w-2xl pb-5 pr-10 text-body-sm text-muted-foreground">
						{item.a}
					</p>
				</Collapsible.Content>
			</Collapsible.Root>
		</li>
	{/each}
</ul>
