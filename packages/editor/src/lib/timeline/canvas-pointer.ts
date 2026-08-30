// Immediate-mode pointer for the canvas timeline (Stage A). The draw pass and
// the hit test are the same pass: a renderer records the rect it draws this
// frame, and tests the pointer against what was drawn LAST frame. One frame of
// latency, and it removes all ordering problems (plan §3).

export interface PointerRegion {
	id: string;
	x: number;
	y: number;
	w: number;
	h: number;
	/** CSS cursor this region claims while hovered. */
	cursor?: string;
}

export class TimelinePointer {
	/** Pointer position in canvas CSS pixels, or -1 when outside. */
	x = -1;
	y = -1;
	down = false;
	/** Claimed each frame; whichever region the pointer is over sets it. */
	cursor = "default";

	private prev: PointerRegion[] = [];
	private next: PointerRegion[] = [];

	set(x: number, y: number): void {
		this.x = x;
		this.y = y;
	}

	clear(): void {
		this.x = -1;
		this.y = -1;
	}

	/** Record a region drawn this frame. */
	region(r: PointerRegion): void {
		this.next.push(r);
	}

	/** The last-drawn region from the PREVIOUS frame under the pointer, or null.
	 *  Last drawn wins, so a region drawn on top claims the hit. */
	hit(): PointerRegion | null {
		for (let i = this.prev.length - 1; i >= 0; i--) {
			const r = this.prev[i];
			if (this.x >= r.x && this.x < r.x + r.w && this.y >= r.y && this.y < r.y + r.h) {
				return r;
			}
		}
		return null;
	}

	/** End of frame: this frame's regions become what the next frame tests. */
	reset(): void {
		const swap = this.prev;
		this.prev = this.next;
		this.next = swap;
		this.next.length = 0;
		this.cursor = "default";
	}
}
