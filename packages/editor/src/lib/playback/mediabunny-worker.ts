/**
 * Worker entry for the MediaBunny decode engine. The body lives in
 * `@recast/media`; this file exists so `new Worker(new URL(...))` has a URL
 * inside THIS app's root, which every bundler resolves without extra config.
 */

import { startMediabunnyWorker } from "@recast/media/playback/worker";

startMediabunnyWorker();
