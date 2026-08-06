<script lang="ts">
import { Pencil } from "@recast/icons";
import { Button } from "@recast/ui/button";
import DialogShell from "@recast/editor/components/dialog/DialogShell.svelte";
import { stemSelectionRange, toErrorMessage } from "./dialog.logic";

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
			const range = stemSelectionRange(seed);
			if (range) {
				inputEl?.setSelectionRange(range[0], range[1]);
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
		error = "Name can't be empty";
		inputEl?.focus();
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
		error = toErrorMessage(e);
		busy = false;
	}
}

function close() {
	open = false;
	onOpenChange(false);
}

function handleKeydown(e: KeyboardEvent) {
	if (e.key === "Enter") {
		e.preventDefault();
		commit();
	}
}
</script>

<DialogShell
	bind:open
	{title}
	subtitle="Extension is preserved if you omit it."
	icon={Pencil}
	{onOpenChange}
>
	<div class="flex flex-col gap-1.5">
		<label
			for="rename-input"
			class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
		>
			{label}
		</label>
		<input
			id="rename-input"
			bind:this={inputEl}
			bind:value
			onkeydown={handleKeydown}
			disabled={busy}
			aria-invalid={error ? "true" : undefined}
			aria-describedby={error ? "rename-error" : undefined}
			class="h-8 w-full rounded-md border border-border/50 bg-input px-2.5 text-[12px] text-foreground outline-none transition-colors focus:border-primary/60 aria-[invalid]:border-destructive/60 disabled:opacity-50"
		/>
		{#if error}
			<!-- role="alert" so the failure is spoken; without it a screen-reader
			     user pressed Enter and got silence. -->
			<p id="rename-error" role="alert" class="text-[11px] text-destructive">{error}</p>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="ghost" size="xs" onclick={close} disabled={busy}>Cancel</Button>
		<Button variant="default" size="xs" onclick={commit} disabled={busy}>
			{busy ? "Saving…" : "Save"}
		</Button>
	{/snippet}
</DialogShell>
