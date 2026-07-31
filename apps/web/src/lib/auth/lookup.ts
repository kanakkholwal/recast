export type EmailStatus = "active" | "pending" | "unknown" | "invalid" | "error";

// Pre-flight lookup for the auth forms: tells them whether an email already has
// an account before they call Better Auth, so "no account here" and "you already
// have one" can be answered with a link instead of a dead-end error toast.
//
// A network blip resolves to `error`, never to a verdict — both callers treat it
// as "proceed and let the real auth call decide", so a flaky lookup can't block
// a genuine sign-in or sign-up.
export async function lookupEmailStatus(email: string): Promise<EmailStatus> {
	try {
		const res = await fetch("/api/auth/lookup", {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ email }),
		});
		if (!res.ok) return "error";
		const data = (await res.json()) as { status: EmailStatus };
		return data.status;
	} catch {
		return "error";
	}
}
