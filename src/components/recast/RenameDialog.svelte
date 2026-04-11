<script lang="ts">
	import { Button } from "$components/ui/button";
	import * as Dialog from "$components/ui/dialog";

	interface Props {
		open: boolean;
		title?: string;
		label?: string;
		initialValue: string;
		/** Called on Save. Throw or reject to keep the dialog open with the error displayed. */
		onSave: (next: string) => void | Promise<void>;
		onOpenChange: (open: boolean) => void;
	}

	let {
		open = $bindable(false),
		title = "Rename",
		label = "New name",
		initialValue,
		onSave,
		onOpenChange,
	}: Props = $props();

	let value = $state("");
	let error = $state<string | null>(null);
	let busy = $state(false);
	let inputEl: HTMLInputElement | null = $state(null);

	// Reset the draft every time the dialog opens for a new target.
	$effect(() => {
		if (open) {
			const seed = initialValue;
			value = seed;
			error = null;
			busy = false;
			// Focus + select the stem (filename without extension) on open.
			queueMicrotask(() => {
				inputEl?.focus();
				const dot = seed.lastIndexOf(".");
				if (dot > 0) {
					inputEl?.setSelectionRange(0, dot);
				} else {
					inputEl?.select();
				}
			});
		}
	});

	async function commit() {
		if (busy) return;
		const trimmed = value.trim();
		if (!trimmed) {
			error = "Name cannot be empty";
			return;
		}
		if (trimmed === initialValue) {
			close();
			return;
		}
		busy = true;
		error = null;
		try {
			await onSave(trimmed);
			close();
		} catch (e) {
			error = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
			busy = false;
		}
	}

	function close() {
		open = false;
		onOpenChange(false);
	}

	function handleKeydown(e: KeyboardEvent) {
		e.stopPropagation();
		if (e.key === "Enter") {
			e.preventDefault();
			commit();
		}
		if (e.key === "Escape") {
			e.preventDefault();
			close();
		}
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
		class="top-[28%] max-w-md translate-y-0 overflow-hidden rounded-xl p-0 ring-1 ring-border"
	>
		<Dialog.Header class="border-b border-border px-4 py-2.5">
			<Dialog.Title class="text-[13px] font-semibold tracking-tight text-foreground">
				{title}
			</Dialog.Title>
			<Dialog.Description class="text-[11px] text-muted-foreground">
				Extension is preserved if you omit it.
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-1.5 px-4 py-3">
			<label for="rename-input" class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
				{label}
			</label>
			<input
				id="rename-input"
				bind:this={inputEl}
				bind:value
				onkeydown={handleKeydown}
				disabled={busy}
				class="h-8 rounded-md border border-input bg-input/30 px-2.5 text-[12px] text-foreground outline-none focus:border-primary disabled:opacity-50"
			/>
			{#if error}
				<p class="text-[11px] text-destructive">{error}</p>
			{/if}
		</div>

		<footer
			class="flex h-10 items-center justify-between gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
		>
			<div class="flex items-center gap-3">
				<span class="flex items-center gap-1">
					<kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">↵</kbd>
					<span>Save</span>
				</span>
				<span class="flex items-center gap-1">
					<kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">esc</kbd>
					<span>Cancel</span>
				</span>
			</div>
			<div class="flex items-center gap-1.5">
				<Button variant="ghost" size="xs" onclick={close} disabled={busy}>Cancel</Button>
				<Button variant="default" size="xs" onclick={commit} disabled={busy}>
					{busy ? "Saving…" : "Save"}
				</Button>
			</div>
		</footer>
	</Dialog.Content>
</Dialog.Root>
