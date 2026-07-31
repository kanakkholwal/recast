/** Deep, de-proxied clone of Svelte `$state` — required before a value crosses
 *  `postMessage`, which throws DataCloneError on a state proxy. */
export function toStatic<T>(value: T): T {
	return $state.snapshot(value) as T;
}
