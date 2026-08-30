// Shared password rules for the signup and reset forms; pure, so the .svelte files wrap these in $derived.

// 0..4 heuristic (length, mixed case, digit, symbol), indexing LABELS and COLORS, which carry a 5th maxed-out entry.
export function scorePasswordStrength(password: string): number {
	let score = 0;
	if (password.length >= 8) score++;
	if (/[A-Z]/.test(password) && /[a-z]/.test(password)) score++;
	if (/\d/.test(password)) score++;
	if (/[^A-Za-z0-9]/.test(password)) score++;
	return score;
}

export const STRENGTH_LABELS = ["Weak", "Fair", "Good", "Strong", "Excellent"];

export const STRENGTH_COLORS = [
	"bg-destructive/60",
	"bg-warning/60",
	"bg-warning",
	"bg-success/80",
	"bg-success",
];

// True while the fields agree or confirm is still empty, so the mismatch hint waits until the user types.
export function passwordsMatch(password: string, confirmPassword: string): boolean {
	return password === confirmPassword || confirmPassword.length === 0;
}

export function canSignUp(input: {
	name: string;
	email: string;
	password: string;
	confirmPassword: string;
	agreed: boolean;
}): boolean {
	return (
		input.name.trim().length > 0 &&
		input.email.trim().length > 0 &&
		input.password.length >= 8 &&
		input.password === input.confirmPassword &&
		input.agreed
	);
}

export function canResetPassword(input: { password: string; confirmPassword: string }): boolean {
	return input.password.length >= 8 && input.password === input.confirmPassword;
}
