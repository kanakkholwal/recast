import type { Provider } from "../types";

/**
 * A provider that drops everything. Used for SSR, tests, when no PostHog key is
 * configured, and as the pre-init target before consent allows a real provider
 * to stand up. Keeps every call site a no-throw call regardless of environment.
 */
const drop = () => undefined;

export const noopProvider: Provider = {
	init: drop,
	capture: drop,
	identify: drop,
	reset: drop,
	captureError: drop,
	register: drop,
	optIn: drop,
	optOut: drop,
	upgradePersistence: drop,
	isFeatureEnabled: () => undefined,
	shutdown: drop,
};
