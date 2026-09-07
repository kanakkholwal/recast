export type EmailStatus = "active" | "pending" | "unknown" | "invalid" | "error";

// Tells the auth forms whether an email already has an account, so both cases get a link instead of a dead-end toast; a network blip resolves to `error`, never a verdict.
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
