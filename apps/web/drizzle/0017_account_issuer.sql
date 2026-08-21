-- better-auth 1.7 keys accounts on (issuer, account_id) instead of provider_id.
-- Added nullable and backfilled first: the column is NOT NULL, and existing rows
-- have no value for it.
ALTER TABLE "account" ADD COLUMN "issuer" text;--> statement-breakpoint

-- Issuers better-auth declares per provider. Anything else is an OAuth provider
-- without an issuer of its own, which gets the synthetic "local:oauth:" form.
UPDATE "account" SET "issuer" = CASE "provider_id"
	WHEN 'credential' THEN 'local:credential'
	WHEN 'google' THEN 'https://accounts.google.com'
	WHEN 'apple' THEN 'https://appleid.apple.com'
	WHEN 'facebook' THEN 'https://www.facebook.com'
	WHEN 'line' THEN 'https://access.line.me'
	ELSE 'local:oauth:' || "provider_id"
END WHERE "issuer" IS NULL;--> statement-breakpoint

ALTER TABLE "account" ALTER COLUMN "issuer" SET NOT NULL;--> statement-breakpoint
CREATE UNIQUE INDEX "account_issuer_account_id_idx" ON "account" USING btree ("issuer","account_id");
