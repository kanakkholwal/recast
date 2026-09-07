<script lang="ts">
import DialogShell from "@recast/editor/components/dialog/DialogShell.svelte";
import { AiWand, ArrowRight } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Markdown } from "@recast/ui/markdown";
import { config } from "$constants/app";
import { groupChanges, KIND_META, LATEST_RELEASE } from "$constants/changelog";
import { whatsNew } from "$lib/stores/whats-new.svelte";

const grouped = $derived(groupChanges(LATEST_RELEASE));

function handleOpenChange(v: boolean) {
	if (!v) whatsNew.dismiss();
	else whatsNew.open = true;
}
</script>

<DialogShell
	open={whatsNew.open}
	onOpenChange={handleOpenChange}
	title={LATEST_RELEASE.title ?? `Recast ${LATEST_RELEASE.version}`}
	subtitle={`What's new · v${LATEST_RELEASE.version} · ${LATEST_RELEASE.date}`}
	icon={AiWand}
	widthClass="sm:max-w-xl"
	bodyClass="max-h-[55vh] scrollbar-transparent"
>
	{#if LATEST_RELEASE.highlights?.length}
		<ul class="mb-4 flex flex-col gap-2">
			{#each LATEST_RELEASE.highlights as h (h)}
				<li
					class="flex items-start gap-2 rounded-lg border border-border/50 bg-muted/20 px-3 py-2 text-[12px] leading-relaxed text-foreground"
				>
					<AiWand class="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
					<span><Markdown inline source={h} /></span>
				</li>
			{/each}
		</ul>
	{/if}

	<div class="flex flex-col gap-4">
		{#each grouped as [kind, items] (kind)}
			{@const meta = KIND_META[kind]}
			{@const Icon = meta.icon}
			<section class="flex flex-col gap-1.5">
				<div class="flex items-center gap-1.5 px-1">
					<Icon class={`size-3.5 ${meta.tone}`} />
					<span class="text-[10.5px] font-medium text-muted-foreground">
						{meta.label}
					</span>
				</div>
				<ul class="flex flex-col gap-1">
					{#each items as it (it)}
						<li
							class="flex items-start gap-2 rounded-md px-1 py-1 text-[12px] leading-relaxed text-foreground/90"
						>
							<span
								class="mt-1.5 size-1 shrink-0 rounded-full bg-foreground/30"
								aria-hidden="true"
							></span>
							<span><Markdown inline source={it} /></span>
						</li>
					{/each}
				</ul>
			</section>
		{/each}
	</div>

	{#snippet footer()}
		<span class="mr-auto text-[11px] text-muted-foreground">{config.appName} · v{config.appVersion}</span>
		<Button variant="ghost" size="sm" href="/whats-new" onclick={() => whatsNew.dismiss()}>
			Full changelog
			<ArrowRight class="ml-1 size-3" />
		</Button>
		<Button size="sm" onclick={() => whatsNew.dismiss()}>Got it</Button>
	{/snippet}
</DialogShell>
