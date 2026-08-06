/**
 * Minimal logger for the editor package. The desktop app has a richer one that
 * ships lines to disk; rather than require it, the editor logs through this and
 * a host can forward by installing a sink.
 */

type Data = Record<string, unknown> | undefined;

export interface LogSink {
	debug(area: string, event: string, data?: Data): void;
	info(area: string, event: string, data?: Data): void;
	warn(area: string, event: string, data?: Data): void;
	error(area: string, event: string, data?: Data): void;
	/** Coalesce a high-frequency line (slider drag, scrub) under `key`; only the
	 *  last one in a quiet window is emitted. */
	debounced(key: string, area: string, event: string, data?: Data): void;
}

const DEBOUNCE_MS = 400;
const pending = new Map<string, ReturnType<typeof setTimeout>>();

const consoleSink: LogSink = {
	debug: (a, e, d) => console.debug(`[${a}] ${e}`, d ?? ""),
	info: (a, e, d) => console.info(`[${a}] ${e}`, d ?? ""),
	warn: (a, e, d) => console.warn(`[${a}] ${e}`, d ?? ""),
	error: (a, e, d) => console.error(`[${a}] ${e}`, d ?? ""),
	debounced(key, a, e, d) {
		clearTimeout(pending.get(key));
		pending.set(
			key,
			setTimeout(() => {
				pending.delete(key);
				this.debug(a, e, d);
			}, DEBOUNCE_MS),
		);
	},
};

let sink: LogSink = consoleSink;

/** Forward editor logs to the host's logger. Returns a restore fn. */
export function setLogSink(next: LogSink): () => void {
	const previous = sink;
	sink = next;
	return () => {
		sink = previous;
	};
}

export const log: LogSink = {
	debug: (a, e, d) => sink.debug(a, e, d),
	info: (a, e, d) => sink.info(a, e, d),
	warn: (a, e, d) => sink.warn(a, e, d),
	error: (a, e, d) => sink.error(a, e, d),
	debounced: (k, a, e, d) => sink.debounced(k, a, e, d),
};
