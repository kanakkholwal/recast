/**
 * How far the browser build may sit from the native goldens.
 *
 * The same numbers the native arm uses, and that is the point: when this run was
 * first measured, every one of the ten fixtures came back BYTE-IDENTICAL to the
 * native wgpu/D3D12 goldens through Chromium's WebGL2/ANGLE stack. Zero
 * differing pixels, not "within tolerance". So this is headroom for a driver's
 * least significant bit, not room the two backends were ever observed to need.
 *
 * `delta.test.ts` holds it honest by checking that a one-pixel row shift still
 * fails. Raise these only with measured numbers in hand, never to green a run.
 */
export const GOLDEN_MAX_CHANNEL = 4;
export const GOLDEN_MAX_MEAN = 0.35;
