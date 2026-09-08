// Module-level cache is safe: the Hyperdrive binding is identical for a Worker's life (Cloudflare's postgres.js example does the same).
let current: App.Platform | undefined;

export function setRequestPlatform(platform: App.Platform | undefined): void {
	if (platform) current = platform;
}

export function getRequestPlatform(): App.Platform | undefined {
	return current;
}
