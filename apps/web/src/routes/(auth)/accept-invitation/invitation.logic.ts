// Eligibility rules for an invitation, derived purely from loader data.

type Invite = { status: string; expired: boolean };
type Viewer = { emailMatches: boolean } | null;

// Signed in, but as the wrong account for this invite.
function sessionMismatch(viewer: Viewer): boolean {
	return Boolean(viewer && !viewer.emailMatches);
}

// Accept/decline are disabled when the viewer can't act on the invite: wrong
// account, expired, or already resolved.
export function isInviteBlocked(invite: Invite, viewer: Viewer): boolean {
	return (
		sessionMismatch(viewer) || invite.expired || invite.status !== "pending"
	);
}
