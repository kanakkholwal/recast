/**
 * Activity-kind → presentation lookup (icon, past-tense verb, tone class). A
 * pure table; importing the @recast/icons icon values here is fine since it's data, not
 * markup. The component owns the header/list rendering.
 */

import { Activity as ActivityIcon, CheckCircle2, Eye, Share2 } from "@recast/icons";
import type { Activity } from "$lib/dashboard/activity";

export const kindMeta: Record<Activity["kind"], { icon: typeof Eye; verb: string; tone: string }> =
	{
		viewed: { icon: Eye, verb: "watched", tone: "text-muted-foreground" },
		completed: { icon: CheckCircle2, verb: "finished", tone: "text-success" },
		shared: { icon: Share2, verb: "shared", tone: "text-primary" },
		downloaded: { icon: ActivityIcon, verb: "downloaded", tone: "text-muted-foreground" },
	};
