/**
 * DashboardSidebar pure helpers: active-route matching and the page.data
 * coercion normalizers. /dashboard/+layout.server.ts surfaces memberships +
 * active org; these fall back to safe defaults when rendered outside that load
 * (e.g. mid route transition). Nav types + defaults stay in the component since
 * they carry Lucide icon values.
 */

export type Membership = {
	organizationId: string;
	name: string;
	role: string;
	plan: string;
};

export type ActiveOrg = { id: string; name: string; role: string; plan: string };

export function isActive(href: string, exact: boolean, currentPath: string): boolean {
	return exact ? currentPath === href : currentPath.startsWith(href);
}

export function resolveMemberships(data: unknown): Membership[] {
	return ((data as { memberships?: Membership[] }).memberships ?? []) as Membership[];
}

export function resolveActiveOrg(data: unknown): ActiveOrg | null {
	return ((data as { activeOrganization?: ActiveOrg }).activeOrganization ??
		null) as ActiveOrg | null;
}
