<script lang="ts">
import { CircleHelp } from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as Tooltip from "@recast/ui/tooltip";

interface Props {
	content: string;
}

let { content }: Props = $props();
</script>

<!-- `child` merges the trigger's props onto our own button. Without it the
     trigger renders a button of its own AROUND this one: nested buttons, two tab
     stops per hint, and the accessible name scraped from the inner content. -->
<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="ghost"
				size="xs"
				class="inline-flex p-0 shrink-0 items-center justify-center rounded-full text-muted-foreground/60 hover:text-foreground"
				aria-label="More information"
			>
				<CircleHelp size={11} />
			</Button>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content class="max-w-56 text-[11px] leading-relaxed">
		{content}
	</Tooltip.Content>
</Tooltip.Root>
