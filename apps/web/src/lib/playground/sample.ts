/**
 * The clip behind "Try the sample". It is the same RAW take the landing page's
 * before/after slider uses as its "before" — so a visitor starts from unpolished
 * footage and does the polishing themselves, which is the whole point.
 *
 * The URL is duplicated from `routes/data.ts` rather than imported: that module
 * pulls ~28 icons and the entire landing dataset, which would undo the
 * playground landing page's code splitting. `sample.test.ts` asserts the two
 * stay equal, so the duplication can't drift.
 */

export const SAMPLE_CLIP = {
	src: "https://acfj680407.ufs.sh/f/04eGlAvZnRytceM29W6qY5xCbfENa2zoprGTi40P83dsVmke",
	/** Shown on the button so the download size isn't a surprise. */
	sizeLabel: "1.7 MB",
	durationLabel: "0:30",
	filename: "recast-sample.mp4",
} as const;
