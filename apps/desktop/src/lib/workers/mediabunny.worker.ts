// Worker entry. The body lives in @recast/media; spawning belongs to the app so
// `new URL(…, import.meta.url)` resolves inside this app's root.
import { startMediabunnyWorker } from "@recast/media/playback/worker";

startMediabunnyWorker();
