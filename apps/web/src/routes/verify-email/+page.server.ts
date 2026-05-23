import { redirect } from "@sveltejs/kit";
import { getAuth } from "$lib/auth/server";
import type { PageServerLoad } from "./$types";

type SessionShape = {
	user: { email: string; emailVerified?: boolean | null };
};

/**
 * Holding page for users whose `emailVerified` is still false. Reached
 * either by the dashboard layout gate or directly via the link in the
 * verification email's "didn't get it?" path. Already-verified users
 * skip past — no point keeping them on this screen.
 */
export const load: PageServerLoad = async ({ request }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	if (!session) redirect(303, "/login?next=/verify-email");
	if (session.user.emailVerified) redirect(303, "/dashboard");

	return { email: session.user.email };
};
