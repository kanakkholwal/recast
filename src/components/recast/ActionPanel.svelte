<script lang="ts">
	import * as Command from "$components/ui/command";
	import * as Dialog from "$components/ui/dialog";
	import { cn } from "$lib/utils";
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
		class="top-1/3 max-w-md translate-y-0 overflow-hidden rounded-xl p-0 ring-1 ring-border"
	>
		<Dialog.Header class="sr-only">
			<Dialog.Title>{title}</Dialog.Title>
			<Dialog.Description>Run an action on the selected item</Dialog.Description>
		</Dialog.Header>
		<Command.Root class="rounded-xl bg-popover">
			<Command.Input placeholder="Search actions..." />
			<Command.List class="max-h-80">
				<Command.Empty>No actions available</Command.Empty>
				<Command.Group heading={title}>
					{#each actions as action, i (action.id)}
						{@const Icon = action.icon}
						<Command.Item
							value={action.id + " " + action.label}
							onSelect={() => runAction(action)}
							class={cn(
								"group/action gap-2",
								action.variant === "destructive" &&
									"data-selected:bg-destructive/10 data-selected:text-destructive"
							)}
						>
							{#if Icon}
								<Icon
									size={16}
									class={cn(
										"shrink-0",
										action.variant === "destructive"
											? "text-destructive"
											: "text-muted-foreground"
									)}
								/>
							{/if}
							<span class="flex-1 truncate">{action.label}</span>
							{#if i === 0}
								<Command.Shortcut>
									<kbd
										class="rounded border border-border bg-background px-1.5 py-0.5 font-mono text-[10px]"
										>↵</kbd
									>
								</Command.Shortcut>
							{:else if i === 1}
								<Command.Shortcut>
									<kbd
										class="rounded border border-border bg-background px-1.5 py-0.5 font-mono text-[10px]"
										>⌘↵</kbd
									>
								</Command.Shortcut>
							{:else if action.shortcut}
								<Command.Shortcut>
									<kbd
										class="rounded border border-border bg-background px-1.5 py-0.5 font-mono text-[10px]"
										>{action.shortcut}</kbd
									>
								</Command.Shortcut>
							{/if}
						</Command.Item>
					{/each}
				</Command.Group>
			</Command.List>
			<div
				class="flex h-9 items-center justify-between border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
			>
				<span class="font-medium">Actions</span>
				<div class="flex items-center gap-3">
					<span class="flex items-center gap-1">
						<kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono"
							>↵</kbd
						>
						<span>Run</span>
					</span>
					<span class="flex items-center gap-1">
						<kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono"
							>esc</kbd
						>
						<span>Close</span>
					</span>
				</div>
			</div>
		</Command.Root>
	</Dialog.Content>
</Dialog.Root>
