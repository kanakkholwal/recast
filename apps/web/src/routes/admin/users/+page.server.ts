import { getAuth } from "$lib/auth/server";
import { requireAdmin } from "$lib/admin/guard";
import type { PageServerLoad } from "./$types";

/**
 * Server-side load that proxies straight to the admin plugin's listUsers
 * endpoint. We pass the caller's headers along so Better Auth can verify
 * the session (the plugin's middleware enforces admin-only access — our
 * `requireAdmin` is belt-and-braces in case the plugin config changes).
 */
export const load: PageServerLoad = async (event) => {
	await requireAdmin(event);

	const url = event.url;
	const limit = Math.min(Math.max(Number(url.searchParams.get("limit") ?? 25), 5), 100);
	const offset = Math.max(Number(url.searchParams.get("offset") ?? 0), 0);
	const searchValue = url.searchParams.get("q")?.trim() ?? "";
	const searchField = (url.searchParams.get("field") as "email" | "name") ?? "email";
	const sortBy = url.searchParams.get("sort") ?? "createdAt";
	const sortDirection =
		url.searchParams.get("dir") === "asc" ? "asc" : ("desc" as const);
	const roleFilter = url.searchParams.get("role")?.trim() || null;
	const statusFilter = url.searchParams.get("status")?.trim() || null;

	const query: Record<string, unknown> = {
		limit,
		offset,
		sortBy,
		sortDirection,
	};
	if (searchValue) {
		query.searchValue = searchValue;
		query.searchField = searchField;
		query.searchOperator = "contains";
	}
	// listUsers supports a single filter — combine with the search query when
	// both are present. Role takes priority over status for the explicit filter.
	if (roleFilter) {
		query.filterField = "role";
		query.filterValue = roleFilter;
		query.filterOperator = "eq";
	} else if (statusFilter) {
		query.filterField = "status";
		query.filterValue = statusFilter;
		query.filterOperator = "eq";
	}

	// The plugin's listUsers types are dynamic; cast to the public shape we
	// actually consume below. Schema-level validation already happened above.
	type ListUsersResult = {
		users: Array<{
			id: string;
			email: string;
			name: string;
			role?: string | null;
			status?: string | null;
			banned?: boolean | null;
			banReason?: string | null;
			banExpires?: Date | string | null;
			createdAt: Date | string;
			emailVerified?: boolean;
		}>;
		total: number;
	};

	const result = (await getAuth().api.listUsers({
		headers: event.request.headers,
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		query: query as any,
	})) as ListUsersResult;

	return {
		users: result.users,
		total: result.total,
		limit,
		offset,
		filters: {
			q: searchValue,
			field: searchField,
			role: roleFilter,
			status: statusFilter,
			sort: sortBy,
			dir: sortDirection,
		},
	};
};
