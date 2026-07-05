// Shared across the waitlist page, the home hero, and pricing — all POST the
// same endpoint. `source` tags where the signup came from for analytics.
export async function joinWaitlist(email: string, source: string): Promise<void> {
	const res = await fetch("/api/waitlist", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ email, source }),
	});
	const data = (await res.json().catch(() => ({}))) as {
		ok?: boolean;
		error?: string;
	};
	if (!data.ok) throw new Error(data.error ?? "Couldn't join the waitlist.");
}
