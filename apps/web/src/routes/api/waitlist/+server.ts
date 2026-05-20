import { json } from "@sveltejs/kit";
import { tryGetDb } from "$lib/db";
import { waitlist } from "$lib/db/schema";
import type { RequestHandler } from "./$types";

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export const POST: RequestHandler = async ({ request }) => {
	let body: { email?: unknown; source?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		return json({ ok: false, error: "Invalid JSON" }, { status: 400 });
	}

	const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
	const source = typeof body.source === "string" ? body.source.slice(0, 64) : null;

	if (!EMAIL_RE.test(email)) {
		return json({ ok: false, error: "Invalid email" }, { status: 400 });
	}

	const db = tryGetDb();
	if (!db) {
		// Placeholder mode — no DATABASE_URL yet. Accept silently so the UI
		// still demos cleanly. Once DB is configured the email persists.
		return json({ ok: true, persisted: false });
	}

	await db
		.insert(waitlist)
		.values({ id: crypto.randomUUID(), email, source })
		.onConflictDoNothing({ target: waitlist.email });

	return json({ ok: true, persisted: true });
};
