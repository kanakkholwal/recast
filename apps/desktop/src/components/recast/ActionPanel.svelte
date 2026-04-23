<script lang="ts">
	import * as Command from "@recast/ui/command";
	import * as Dialog from "@recast/ui/dialog";
	import { cn } from "@recast/ui/utils";
	import type { RecastAction } from "./types";

	interface Props {
		open: boolean;
		actions: RecastAction[];
		title?: string;
		onOpenChange: (open: boolean) => void;
	}

	let { open = $bindable(false), actions, title = "Actions", onOpenChange }: Props = $props();

	function runAction(action: RecastAction) {
		onOpenChange(false);
		queueMicrotask(() => action.onAction());
	}
</script>

<Dialog.Root
	bind:open
	onOpenChange={(v) => {
		open = v;
		onOpenChange(v);
	}}
>
	<Dialog.Content
		showCloseButton={false}
		class="top-1/3 max-w-md translate-y-0 overflow-hidden rounded-[28px] p-0 shadow-craft-floating border-none bg-transparent"
	>
		<Dialog.Header class="sr-only">
			<Dialog.Title>{title}</Dialog.Title>
			<Dialog.Description>Run an action on the selected item</Dialog.Description>
		</Dialog.Header>
		
    <Command.Root class="rounded-[28px] bg-background/80 backdrop-blur-3xl ring-1 ring-inset ring-border/60 shadow-(--shadow-craft-inset-strong) overflow-hidden font-sans select-none">
			<Command.Input placeholder="Search actions..." class="h-11 border-none bg-transparent text-[13px] font-medium px-5" />
			<Command.List class="max-h-80 px-2 py-2 scrollbar-transparent">
				<Command.Empty class="py-12 text-center text-[12px] font-medium text-foreground/40">No actions available</Command.Empty>
				<Command.Group heading={title}>
					{#each actions as action, i (action.id)}
						{@const Icon = action.icon}
						<Command.Item
							value={action.id + " " + action.label}
							onSelect={() => runAction(action)}
							class={cn(
								"group/action h-10 gap-3.5 rounded-xl px-3.5 transition-all duration-200",
								action.variant === "destructive" &&
									"data-selected:bg-destructive/10 data-selected:text-destructive"
							)}
						>
							{#if Icon}
								<Icon
									size={14}
									class={cn(
										"shrink-0",
										action.variant === "destructive"
											? "text-destructive"
											: "text-foreground/30 group-data-[selected=true]/action:text-foreground/80 group-hover/action:text-foreground/80"
									)}
								/>
							{/if}
							<span class="flex-1 truncate text-[13px] font-semibold text-foreground/70 group-data-[selected=true]/action:text-foreground group-hover/action:text-foreground transition-colors">{action.label}</span>
							
              <div class="flex items-center gap-1.5">
                {#if i === 0}
                  <Command.Shortcut class="invisible-ui opacity-0 group-data-[selected=true]/action:opacity-100 transition-all duration-300">
                    <kbd class="rounded-md border border-border-subtle bg-background px-1.5 py-0.5 font-mono text-[9px] font-bold text-foreground/40 shadow-craft-sm">↵</kbd>
                  </Command.Shortcut>
                {:else if i === 1}
                  <Command.Shortcut class="invisible-ui opacity-0 group-data-[selected=true]/action:opacity-100 transition-all duration-300">
                    <kbd class="rounded-md border border-border-subtle bg-background px-1.5 py-0.5 font-mono text-[9px] font-bold text-foreground/40 shadow-craft-sm">⌘↵</kbd>
                  </Command.Shortcut>
                {:else if action.shortcut}
                  <Command.Shortcut class="invisible-ui opacity-0 group-data-[selected=true]/action:opacity-100 transition-all duration-300">
                    <kbd class="rounded-md border border-border-subtle bg-background px-1.5 py-0.5 font-mono text-[9px] font-bold text-foreground/40 shadow-craft-sm">{action.shortcut}</kbd>
                  </Command.Shortcut>
                {/if}
              </div>
						</Command.Item>
					{/each}
				</Command.Group>
			</Command.List>
			
      <footer
				class="mx-4 mb-4 flex h-9 items-center justify-between rounded-full px-5 text-[9px] font-bold uppercase tracking-[0.12em] text-foreground/30"
			>
				<span>Available Actions</span>
				<div class="flex items-center gap-4">
					<span class="flex items-center gap-1.5">
						<kbd class="opacity-50 text-[10px]">↵</kbd>
						<span>Run</span>
					</span>
					<span class="flex items-center gap-1.5">
						<kbd class="opacity-50 text-[10px]">esc</kbd>
						<span>Close</span>
					</span>
				</div>
			</footer>
		</Command.Root>
	</Dialog.Content>
</Dialog.Root>
