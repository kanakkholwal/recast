type SessionShape = {
	data: {
		session?: { impersonatedBy?: string | null } | null;
		user?: { email?: string | null; name?: string | null } | null;
	} | null;
};

/** Pure coercion of the auth session into the fields the banner renders. */
export function readImpersonation(session: unknown): {
	impersonatedBy: string | null;
	targetEmail: string;
} {
	const s = session as SessionShape;
	return {
		impersonatedBy: s.data?.session?.impersonatedBy ?? null,
		targetEmail: s.data?.user?.email ?? "user",
	};
}
