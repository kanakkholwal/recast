export type EmailStatus = "active" | "pending" | "unknown" | "invalid";

// Pre-flight lookup for the login form: decides whether the email is eligible
// to sign in before we call Better Auth. A network blip resolves to `active`
// so a flaky lookup never blocks a genuine sign-in attempt.
export async function lookupEmailStatus(email: string): Promise<EmailStatus> {
	try {
		const res = await fetch("/api/auth/lookup", {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ email }),
		});
		const data = (await res.json()) as { status: EmailStatus };
		return data.status;
	} catch {
		return "active";
	}
}
