/**
 * `PersistedState` — a Svelte 5 rune-module port of the React
 * `useSyncExternalStore`-backed `useStorage` hook. One reactive value, one
 * storage key, kept in sync across every Tauri window / browser tab that
 * shares the same storage origin.
 *
 * Usage (idiomatic Svelte — the value lives on `.current`, like `runed`):
 *
 *   const flags = new PersistedState("recast-experimental-flags", DEFAULTS);
 *   flags.current;            // reactive read (templates, $derived, $effect)
 *   flags.current = next;     // writes through to storage + broadcasts
 *   flags.reset();            // clear the key, revert to the initial value
 *
 * For non-reactive call sites (SDK bootstrap, install-id reads, one-shot
 * route reads) use the `safeStorage` helper at the bottom — same null /
 * parse / quota safety, no reactivity overhead.
 *
 * Robustness contract — every one of these is handled, none of them throw:
 *   - missing key / `null`        → the initial value
 *   - no `window` (SSR / preview) → the initial value, storage untouched
 *   - malformed JSON / bad data   → the initial value, `onError` fired
 *   - partial / stale object shape → shallow-merged over the initial, so a
 *                                    field added after the value was written
 *                                    keeps its default instead of going
 *                                    `undefined`
 *   - quota / private-mode writes  → swallowed best-effort
 */

type StorageArea = "local" | "session";

export interface Serializer<T> {
	serialize: (value: T) => string;
	deserialize: (raw: string) => T;
}

export type PersistedErrorContext = "read" | "deserialize" | "write" | "remove";

export interface PersistedStateOptions<T> {
	/** Which web storage to back onto. Defaults to `"local"`. */
	storage?: StorageArea;
	/**
	 * Listen for writes from other windows/tabs (and other instances in this
	 * same document) and re-read when the key changes. Defaults to `true`.
	 */
	syncTabs?: boolean;
	/**
	 * Override how the value is (de)serialized. Defaults to a type-aware
	 * serializer inferred from `initialValue` — strings stored raw, numbers /
	 * booleans coerced, everything else JSON. Inferring from the initial value
	 * keeps backward-compatibility with keys that were written as raw strings.
	 */
	serializer?: Serializer<T>;
	/**
	 * Observe parse / quota failures instead of letting them pass silently.
	 * The value still falls back safely; this is purely for telemetry.
	 */
	onError?: (error: unknown, context: PersistedErrorContext, key: string) => void;
}

const SAME_DOC_EVENT = "recast:persisted-state";

interface SameDocDetail {
	key: string;
	area: StorageArea;
	source: number;
}

let instanceCounter = 0;

const isBrowser = typeof window !== "undefined";

function isPlainObject(value: unknown): value is Record<string, unknown> {
	if (typeof value !== "object" || value === null) return false;
	const proto = Object.getPrototypeOf(value);
	return proto === Object.prototype || proto === null;
}

/**
 * Build a serializer from the *initial* value's runtime type. This mirrors the
 * React hook's branching and, crucially, keeps reading values that earlier
 * code wrote as raw (un-quoted) strings.
 */
export function inferSerializer<T>(initialValue: T): Serializer<T> {
	const type = typeof initialValue;

	if (type === "string") {
		return {
			serialize: (v) => v as unknown as string,
			deserialize: (raw) => raw as unknown as T,
		};
	}

	if (type === "number") {
		return {
			serialize: (v) => String(v),
			deserialize: (raw) => {
				const parsed = Number(raw);
				return (Number.isNaN(parsed) ? initialValue : parsed) as T;
			},
		};
	}

	if (type === "boolean") {
		return {
			serialize: (v) => String(v),
			deserialize: (raw) => (raw === "true") as unknown as T,
		};
	}

	// object | array | null | bigint | symbol | function → JSON, with a couple
	// of shape guards so genuinely corrupt data falls back instead of poisoning
	// the reactive value.
	return {
		serialize: (v) => JSON.stringify(v),
		deserialize: (raw) => {
			const parsed: unknown = JSON.parse(raw);
			// An array key that deserialized to a non-array is corrupt — bail to
			// the initial via the caller's catch.
			if (Array.isArray(initialValue) && !Array.isArray(parsed)) {
				throw new TypeError("expected an array");
			}
			// Stale / partial object shapes keep their defaults for missing keys.
			if (isPlainObject(initialValue) && isPlainObject(parsed)) {
				return { ...(initialValue as object), ...(parsed as object) } as T;
			}
			return parsed as T;
		},
	};
}

export class PersistedState<T> {
	#key: string;
	#initial: T;
	#storage: StorageArea;
	#serializer: Serializer<T>;
	#syncTabs: boolean;
	#onError?: PersistedStateOptions<T>["onError"];
	#id = ++instanceCounter;

	#current = $state<T>() as T;

	#onStorage?: (event: StorageEvent) => void;
	#onSameDoc?: (event: Event) => void;

	constructor(key: string, initialValue: T, options: PersistedStateOptions<T> = {}) {
		this.#key = key;
		this.#initial = initialValue;
		this.#storage = options.storage ?? "local";
		this.#serializer = options.serializer ?? inferSerializer(initialValue);
		this.#syncTabs = options.syncTabs ?? true;
		this.#onError = options.onError;

		// Read the persisted value synchronously so the very first reactive read
		// already reflects storage — no flash of the default on hydrate.
		this.#current = this.#read();

		if (isBrowser && this.#syncTabs) this.#subscribe();
	}

	get current(): T {
		return this.#current;
	}

	set current(value: T) {
		this.#current = value;
		this.#write(value);
		this.#broadcast();
	}

	/** Clear the stored value and revert the reactive value to the initial. */
	reset(): void {
		this.#current = this.#initial;
		if (isBrowser) {
			try {
				this.#area().removeItem(this.#key);
			} catch (error) {
				this.#onError?.(error, "remove", this.#key);
			}
		}
		this.#broadcast();
	}

	/**
	 * Detach the cross-context listeners. Only needed for component-scoped
	 * instances — module-level singletons live for the page's lifetime and
	 * never need teardown.
	 */
	dispose(): void {
		if (!isBrowser) return;
		if (this.#onStorage) window.removeEventListener("storage", this.#onStorage);
		if (this.#onSameDoc) window.removeEventListener(SAME_DOC_EVENT, this.#onSameDoc);
		this.#onStorage = undefined;
		this.#onSameDoc = undefined;
	}

	#area(): Storage {
		return this.#storage === "session" ? window.sessionStorage : window.localStorage;
	}

	#read(): T {
		if (!isBrowser) return this.#initial;

		let raw: string | null;
		try {
			raw = this.#area().getItem(this.#key);
		} catch (error) {
			// Storage disabled entirely (some private modes throw on access).
			this.#onError?.(error, "read", this.#key);
			return this.#initial;
		}

		if (raw === null) return this.#initial;

		try {
			return this.#serializer.deserialize(raw);
		} catch (error) {
			this.#onError?.(error, "deserialize", this.#key);
			return this.#initial;
		}
	}

	#write(value: T): void {
		if (!isBrowser) return;
		try {
			this.#area().setItem(this.#key, this.#serializer.serialize(value));
		} catch (error) {
			// Quota exceeded / private mode — best effort, the rune still updated.
			this.#onError?.(error, "write", this.#key);
		}
	}

	/** Tell same-document instances of this key to re-read. */
	#broadcast(): void {
		if (!isBrowser || !this.#syncTabs) return;
		const detail: SameDocDetail = { key: this.#key, area: this.#storage, source: this.#id };
		window.dispatchEvent(new CustomEvent(SAME_DOC_EVENT, { detail }));
	}

	#subscribe(): void {
		// Cross-document: native `storage` events fire in *other* windows/tabs
		// that share this origin (Tauri v2 webviews do). `event.key` is null on a
		// `clear()`, which we also honour by re-reading.
		this.#onStorage = (event: StorageEvent) => {
			if (event.key !== null && event.key !== this.#key) return;
			if (event.storageArea && event.storageArea !== this.#area()) return;
			this.#current = this.#read();
		};

		// Same-document: native `storage` does NOT fire in the window that wrote
		// it, so a second instance of the same key in this document wouldn't see
		// the change. The custom channel covers that; the source guard stops an
		// instance from reacting to its own write.
		this.#onSameDoc = (event: Event) => {
			const detail = (event as CustomEvent<SameDocDetail>).detail;
			if (!detail || detail.key !== this.#key || detail.area !== this.#storage) return;
			if (detail.source === this.#id) return;
			this.#current = this.#read();
		};

		window.addEventListener("storage", this.#onStorage);
		window.addEventListener(SAME_DOC_EVENT, this.#onSameDoc);
	}
}

/**
 * Convenience factory for when a `new` reads awkwardly at the call site.
 * Returns the same `PersistedState` instance — read/write via `.current`.
 */
export function persisted<T>(
	key: string,
	initialValue: T,
	options?: PersistedStateOptions<T>,
): PersistedState<T> {
	return new PersistedState(key, initialValue, options);
}

interface SafeStorageOptions<T> {
	storage?: StorageArea;
	serializer?: Serializer<T>;
	onError?: (error: unknown, context: PersistedErrorContext, key: string) => void;
}

function area(storage: StorageArea): Storage {
	return storage === "session" ? window.sessionStorage : window.localStorage;
}

/**
 * Non-reactive twin of `PersistedState` for call sites that just need a safe
 * read/write — SDK bootstrap, anonymous install-id, one-shot route reads —
 * without paying for a reactive rune or cross-tab listeners. Same null / parse
 * / quota guarantees.
 */
export const safeStorage = {
	get<T>(key: string, fallback: T, options: SafeStorageOptions<T> = {}): T {
		if (!isBrowser) return fallback;
		const storage = options.storage ?? "local";
		const serializer = options.serializer ?? inferSerializer(fallback);

		let raw: string | null;
		try {
			raw = area(storage).getItem(key);
		} catch (error) {
			options.onError?.(error, "read", key);
			return fallback;
		}
		if (raw === null) return fallback;

		try {
			return serializer.deserialize(raw);
		} catch (error) {
			options.onError?.(error, "deserialize", key);
			return fallback;
		}
	},

	set<T>(key: string, value: T, options: SafeStorageOptions<T> = {}): void {
		if (!isBrowser) return;
		const storage = options.storage ?? "local";
		const serializer = options.serializer ?? inferSerializer(value);
		try {
			area(storage).setItem(key, serializer.serialize(value));
		} catch (error) {
			options.onError?.(error, "write", key);
		}
	},

	remove(
		key: string,
		options: Pick<SafeStorageOptions<unknown>, "storage" | "onError"> = {},
	): void {
		if (!isBrowser) return;
		const storage = options.storage ?? "local";
		try {
			area(storage).removeItem(key);
		} catch (error) {
			options.onError?.(error, "remove", key);
		}
	},
};
