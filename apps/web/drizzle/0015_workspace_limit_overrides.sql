ALTER TABLE "organization" ADD COLUMN "seat_limit" integer;--> statement-breakpoint
ALTER TABLE "organization" ADD COLUMN "storage_limit_bytes" bigint;--> statement-breakpoint
ALTER TABLE "organization" ADD COLUMN "delivery_limit_bytes" bigint;--> statement-breakpoint
ALTER TABLE "organization" ADD COLUMN "active_recasts_limit" integer;