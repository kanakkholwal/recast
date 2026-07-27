import { serverEnv } from "$lib/env/server";
import type { PlanId } from "./catalog";

export * from "./catalog";

/** Polar product ids — set in the Polar dashboard, then in .env. */
export function polarProductIdFor(plan: PlanId): string | null {
	switch (plan) {
		case "pro":
			return serverEnv().POLAR_PRODUCT_ID_PRO ?? null;
		// Enterprise is contract-billed; free has nothing to charge.
		case "enterprise":
		case "free":
			return null;
	}
}
