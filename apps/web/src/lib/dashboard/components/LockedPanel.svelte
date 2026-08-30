<script lang="ts">
import { ArrowRight, Lock } from "@recast/icons";
import { Button } from "@recast/ui/button";
import type { Snippet } from "svelte";

// A Pro-only panel: the blurred layer is skeleton geometry, never invented numbers, since the data isn't fetched.
let {
	title,
	description,
	href = "/dashboard/settings/billing",
	cta = "Upgrade to Pro",
	preview,
}: {
	title: string;
	description: string;
	href?: string;
	cta?: string;
	preview: Snippet;
} = $props();
</script>

<section class="surface relative overflow-hidden">
	<div
		aria-hidden="true"
		class="pointer-events-none select-none p-5 opacity-60 blur-[6px] motion-reduce:blur-[6px]"
	>
		{@render preview()}
	</div>

	<div
		class="absolute inset-0 grid place-items-center bg-gradient-to-b from-card/60 via-card/85 to-card p-6 text-center"
	>
		<div class="flex max-w-xs flex-col items-center">
			<Lock class="size-5 text-muted-foreground" />
			<h3 class="mt-3 font-display text-body font-medium text-foreground">{title}</h3>
			<p class="mt-1 text-body-sm text-muted-foreground">{description}</p>
			<Button {href} size="sm" variant="dark" class="group/cta mt-4 gap-2">
				{cta}
				<ArrowRight
					class="size-3.5 transition-transform group-hover/cta:translate-x-0.5 motion-reduce:transition-none"
				/>
			</Button>
		</div>
	</div>
</section>
