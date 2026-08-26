// Ambient module shims for untyped dependencies that the project uses but
// don't ship .d.ts files. Each shim is intentionally loose (`any`) — the
// imports are exercised at runtime via @recast/media's well-typed surface.
//
// `gifenc`: GIF encoder used by `@recast/media/encoders` (PR-B). Pure JS;
// treat as `any` for type-checking.
declare module "gifenc";
