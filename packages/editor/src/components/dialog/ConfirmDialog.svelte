<script lang="ts">
import { AlertTriangle } from "@recast/icons";
import { Button } from "@recast/ui/button";
import DialogShell from "./DialogShell.svelte";
import { toErrorMessage } from "./dialog.logic";

interface Props {
	open: boolean;
	title: string;
	description?: string;
	confirmLabel?: string;
	cancelLabel?: string;
	variant?: "default" | "destructive";
	/** Called on confirm. Throw or reject to keep the dialog open with the error displayed. */
	onConfirm: () => void | Promise<void>;
	onOpenChange: (open: boolean) => void;
}

let {
	open = $bindable(false),
	title,
	description,
	confirmLabel = "Confirm",
	cancelLabel = "Cancel",
	variant = "default",
	onConfirm,
	onOpenChange,
}: Props = $props();

let error = $state<string | null>(null);
let busy = $state(false);
let cancelEl = $state<HTMLButtonElement | null>(null);

$effect(() => {
	if (open) {
		error = null;
		busy = false;
		// Cancel takes focus, so Enter on a destructive prompt backs out; accepting still needs one deliberate move.
		queueMicrotask(() => cancelEl?.focus());
	}
});

async function confirm() {
	if (busy) return;
	busy = true;
	error = null;
	try {
		await onConfirm();
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

// On the window, not a wrapper: the footer buttons are siblings of the body, where focus starts on Cancel.
function onWindowKeydown(e: KeyboardEvent) {
	if (!open || busy) return;
	if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
		e.preventDefault();
		confirm();
	}
}
</script>

<svelte:window onkeydown={onWindowKeydown} />

<DialogShell
	bind:open
	{title}
	icon={variant === "destructive" ? AlertTriangle : undefined}
	tone={variant === "destructive" ? "destructive" : "default"}
	{onOpenChange}
>
	<p class="max-w-[46ch] text-[12px] leading-[1.55] text-muted-foreground text-pretty">
		{description ?? "This can't be undone."}
	</p>
	{#if error}
		<p class="mt-2 text-[12px] text-destructive" role="alert">{error}</p>
	{/if}

	{#snippet footer()}
		<Button bind:ref={cancelEl} variant="ghost" size="sm" onclick={close} disabled={busy}>
			{cancelLabel}
		</Button>
		<Button
			variant={variant === "destructive" ? "destructive" : "default"}
			size="sm"
			onclick={confirm}
			disabled={busy}
		>
			{busy ? "Working…" : confirmLabel}
		</Button>
	{/snippet}
</DialogShell>
