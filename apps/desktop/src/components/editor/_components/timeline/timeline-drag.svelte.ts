import { getContext, setContext } from "svelte";

// Which lane card is under an active pointer gesture. Read by the timeline to
// pin that card's row for the length of the drag; the layout otherwise re-packs
// on every pointer move and the card jumps rows out from under the cursor.
//
// Context, not the store: this is per-gesture view state, and the store's
// snapshot/undo/serialization has no business seeing it.

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
