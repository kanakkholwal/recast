// Bundler-injected members, declared locally rather than pulling in `vite/client`, which this package doesn't depend on.
interface ImportMetaEnv {
	readonly DEV: boolean;
	readonly PROD: boolean;
	readonly [key: string]: string | boolean | undefined;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
	glob<T = unknown>(
		pattern: string,
		options?: { query?: string; import?: string; eager?: boolean },
	): Record<string, T>;
}

// gifenc ships no type declarations.
declare module "gifenc";
