// Worker entry: the body lives in @recast/media, but spawning belongs to the app so `import.meta.url` resolves here.
import { startMediabunnyWorker } from "@recast/media/playback/worker";

startMediabunnyWorker();
