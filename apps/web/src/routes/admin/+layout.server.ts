import { requireAdmin } from "$lib/admin/guard";
import type { LayoutServerLoad } from "./$types";

export const load: LayoutServerLoad = async (event) => {
	const session = await requireAdmin(event);
	return {
		admin: {
			id: session.user.id,
			email: session.user.email,
			name: session.user.name,
			impersonatedBy: session.session.impersonatedBy,
		},
	};
};
