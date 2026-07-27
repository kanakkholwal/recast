ALTER TYPE "public"."plan" ADD VALUE IF NOT EXISTS 'enterprise';--> statement-breakpoint
CREATE TABLE IF NOT EXISTS "billing_checkout_intent" (
	"user_id" text PRIMARY KEY NOT NULL,
	"organization_id" text NOT NULL,
	"seats" integer DEFAULT 3 NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "billing_checkout_intent" ADD CONSTRAINT "billing_checkout_intent_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "billing_checkout_intent" ADD CONSTRAINT "billing_checkout_intent_organization_id_organization_id_fk" FOREIGN KEY ("organization_id") REFERENCES "public"."organization"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint

ALTER TABLE "workspace_usage" ADD COLUMN "delivery_bytes_this_month" bigint DEFAULT 0 NOT NULL;--> statement-breakpoint
ALTER TABLE "workspace_usage" ADD COLUMN "delivery_period_start" timestamp DEFAULT now() NOT NULL;--> statement-breakpoint
CREATE INDEX "workspace_usage_delivery_idx" ON "workspace_usage" USING btree ("delivery_period_start");--> statement-breakpoint

ALTER TABLE "subscription" ADD COLUMN "seats" integer DEFAULT 3 NOT NULL;--> statement-breakpoint
-- Subscriptions move from per-user to per-workspace. Added nullable so the
-- backfill below can attribute existing rows before the NOT NULL is enforced.
ALTER TABLE "subscription" ADD COLUMN "organization_id" text;--> statement-breakpoint

-- Attribute each existing subscription to its buyer's oldest owned workspace.
UPDATE "subscription" AS s
SET "organization_id" = o."organization_id"
FROM (
	SELECT DISTINCT ON (m."user_id") m."user_id", m."organization_id"
	FROM "member" m
	WHERE m."role" = 'owner'
	ORDER BY m."user_id", m."created_at" ASC
) AS o
WHERE s."user_id" = o."user_id";--> statement-breakpoint

-- Rows we cannot attribute (buyer owns no workspace) are dropped; this table
-- is a mirror, and the next Polar webhook re-creates them correctly.
DELETE FROM "subscription" WHERE "organization_id" IS NULL;--> statement-breakpoint

-- Keep the newest row per workspace so the unique constraint can be added:
-- two co-owners of one workspace could otherwise collide here.
DELETE FROM "subscription" s
USING "subscription" dup
WHERE s."organization_id" = dup."organization_id"
  AND s."updated_at" < dup."updated_at";--> statement-breakpoint

ALTER TABLE "subscription" ALTER COLUMN "organization_id" SET NOT NULL;--> statement-breakpoint
ALTER TABLE "subscription" DROP CONSTRAINT IF EXISTS "subscription_user_id_unique";--> statement-breakpoint
ALTER TABLE "subscription" ADD CONSTRAINT "subscription_organization_id_organization_id_fk" FOREIGN KEY ("organization_id") REFERENCES "public"."organization"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "subscription" ADD CONSTRAINT "subscription_organization_id_unique" UNIQUE("organization_id");--> statement-breakpoint
CREATE INDEX "subscription_user_idx" ON "subscription" USING btree ("user_id");--> statement-breakpoint

-- Entitlements are read from organization.plan; mirror any already-paid
-- subscription onto its workspace so nobody silently loses access.
UPDATE "organization" o
SET "plan" = s."plan"::text
FROM "subscription" s
WHERE s."organization_id" = o."id"
  AND s."status" IN ('active', 'trialing')
  AND s."plan" <> 'free'
  AND o."plan" = 'free';
