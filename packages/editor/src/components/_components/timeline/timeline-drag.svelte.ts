import { getContext, setContext } from "svelte";

// Pins the dragged card's row for the gesture, or the layout re-packs on every move and the card jumps out from under the cursor. Context, not the store: per-gesture view state has no place in undo or serialization.

const KEY = Symbol("timeline-lane-drag");

export class LaneDragState {
	/** Id of the card being dragged, or null. */
	cardId = $state<string | null>(null);

	begin(id: string) {
		this.cardId = id;
	}

	end() {
		this.cardId = null;
	}
}

export function provideLaneDrag(): LaneDragState {
	const state = new LaneDragState();
	setContext(KEY, state);
	return state;
}

/** Null outside a timeline (a card rendered standalone in a test or story). */
export function useLaneDrag(): LaneDragState | null {
	return getContext<LaneDragState | undefined>(KEY) ?? null;
}
