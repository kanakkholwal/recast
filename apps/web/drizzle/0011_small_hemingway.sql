ALTER TABLE "share_reaction" DROP CONSTRAINT "share_reaction_unique_key";--> statement-breakpoint
ALTER TABLE "share_reaction" ADD COLUMN "ip_hash" text;--> statement-breakpoint
ALTER TABLE "share_reaction" ADD CONSTRAINT "share_reaction_reactor_key" UNIQUE("share_slug","ip_hash");