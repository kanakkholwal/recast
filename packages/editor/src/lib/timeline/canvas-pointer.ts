// Immediate-mode pointer (Stage A): draw and hit-test are one pass — records the rect drawn this frame, tests the pointer against last frame's (one-frame latency, removes ordering problems; plan §3).

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
