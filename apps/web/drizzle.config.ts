import { defineConfig } from "drizzle-kit";

// drizzle-kit auto-loads .env from cwd, so calling it elsewhere needs --config or an exported env.
if (!process.env.DATABASE_URL) {
	throw new Error(
		"DATABASE_URL is not set — copy .env.example to .env and set it before running drizzle-kit.",
	);
}

export default defineConfig({
	dialect: "postgresql",
	schema: "./src/lib/db/schema/*",
	out: "./drizzle",
	dbCredentials: { url: process.env.DATABASE_URL },
	strict: true,
	verbose: true,
});
