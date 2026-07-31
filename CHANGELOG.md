# Changelog

All notable changes to Recast are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is the **canonical source for both** the GitHub release notes and
the in-app "What's new" panel:

- **Releases** — `Release Desktop App` workflow runs
  `scripts/extract-changelog.mjs <tag>` and uses the matching
  `## [<version>]` section as the release body.
- **Desktop in-app** —
  [`apps/desktop/src/constants/changelog.ts`](apps/desktop/src/constants/changelog.ts)
  is **regenerated** from this file by `pnpm sync:changelog` (and
  automatically before each `pnpm dev` / `pnpm build` of the desktop app).
  Don't edit the `RELEASES` array directly — it lives between
  `RELEASES:START` / `RELEASES:END` markers and will be overwritten.
- **Web** — `apps/web/src/routes/changelog/+page.ts` reads from the
  GitHub Releases REST API at runtime, which means the same curated section
  surfaces there too as soon as the release publishes.

Headings must follow the literal form `## [<version>] — <date>` (em-dash) so
both the extractor and the desktop sync can find them. Subsections use
`### Added`, `### Changed`, `### Fixed`, `### Deprecated`. An optional
`### Highlights` block above those is rendered as the "punchy" bullet row in
the desktop dialog.

## Authoring entries

Add a changeset per PR instead of editing this file by hand for in-flight
work:

```sh
pnpm changeset
```

See [`.changeset/README.md`](.changeset/README.md) for the full flow.
`pnpm release:prepare <version>` consumes pending changesets and the current
`[Unreleased]` block into a new dated section.

## [Unreleased]

## [0.4.4] — 2026-07-30

### Highlights
- Roll, slide, and slip editing on the video track, so you can move a cut point or shift footage inside its slot without dragging both neighbours by hand.
- Redesigned timeline blocks: solid, named objects at one consistent height, with track colours that follow editing conventions.
- A resizable timeline, with the ruler and track names staying put while you scroll.

### Added
- Roll, slide, and slip editing on the video track. Drag a split to move the cut point between two clips, drag a removed section to shift it without changing its length, and Alt+drag a clip that has a removed section on both sides to slide its footage inside its slot.
- Fine control while dragging anything on the timeline. Hold Shift for slow, precise movement, and hold Ctrl (Cmd on macOS) to ignore snapping when you need a position the guides keep pulling you off.
- Clip names on the timeline. Video blocks show the source file and their length, and the audio track names what it captured, so a split clip and its neighbour are no longer identical grey blocks.
- A resizable timeline: drag its top edge, or focus the handle and use the arrow keys, to trade height between the timeline and the preview. It stops well short of taking over the window, and the height is remembered between sessions.

### Changed
- Redesigned the timeline blocks. Clips are solid, readable objects with their name inside them, at one consistent height across every track, instead of faint tinted strips.
- New timeline track colours, picked to match editing conventions rather than the brand: red for removed sections, green for the recording's audio, teal for music, blue for zoom, violet for markup.
- The waveform now appears only in the Audio track. It used to be redrawn faintly behind the Cuts track whenever you hid the Audio one, which put it back on screen at the moment you asked for it to go away.
- The Camera panel now says why there is no camera to edit: the camera was off, it was switched on but failed to record, or the recording was made before Recast could capture a camera at all.
- The Zoom and Markup tracks now appear once they hold something and stay out of the way while empty, so a fresh recording opens with a shorter timeline and a bigger preview. Turning either on or off in the Layers menu keeps your choice.
- The mouse wheel now scrolls the timeline down to tracks that do not fit, instead of only ever scrolling sideways. Shift+wheel still pans, Ctrl (Cmd on macOS) still zooms, and with nothing below the fold the wheel pans as before.
- The time ruler and the track names now stay put while you scroll the timeline. The ruler holds at the top as the tracks scroll under it, and the names stay pinned at the left as you scroll sideways, so you never lose track of where you are or which track you are looking at.
- Easing is now picked from named presets with a live preview, with the curve editor tucked behind a Custom option, and the same control is used everywhere easing appears.

### Fixed
- Exporting a project whose zoom dips below 1x could crash partway through encoding. The frame was being scaled smaller than the region the encoder was told to read from, so it read past the end of it.
- If the hardware encoder fails mid-export, the export now falls back to software encoding instead of failing outright.
- Uploading a finished export to Recast Cloud or Google Drive looked like nothing had happened. The buttons now show that the click landed and go quiet while the transfer runs, so a single export can no longer be uploaded three times because it seemed unresponsive. Once it lands, the same button copies the link, and a failed one offers a retry.
- Sharing an export to Recast Cloud from the editor never opened its progress window; it only appeared in the activity centre. It now opens the same window a share from the Exports page does.
- Dragging a zoom or markup block on the timeline barely moved it. The drag maths mixed up pixels and seconds, so a block travelled a hundredth of the distance your pointer did, which read as the block being stuck.
- Timeline blocks jumped to a different row mid-drag as soon as they touched a neighbour, leaving the block you were holding somewhere other than under the cursor.
- Clicking a timeline block to select it could nudge it slightly and add an undo step that changed nothing.
- The preview's progress bar lagged behind its own handle, and stepped forward in visible jumps during playback instead of moving smoothly.
- The preview had no scrubber at all in fullscreen when the timeline was open.
- Very short blocks were nearly impossible to grab by the edge to resize.

## [0.4.3] — 2026-07-27

### Highlights
- Record your webcam alongside your screen and drop it in as a floating bubble you can move, resize, shape, and style. It grows and drifts out of the way when you zoom in.
- A rebuilt preview engine: playback, scrubbing, and seeking are smoother, large recordings load faster, and it recovers on its own if a frame fails to decode.
- Balance your audio: set the system and microphone levels independently, even out overall loudness on export, and add background music.

### Added
- Camera overlay. Your webcam records as its own track and appears as a bubble in the editor. Move and resize it, pick a square, rounded, or circle shape, mirror it, and give it a drop shadow. It renders in the preview and the exported video.
- Grow the camera on zoom. As a zoom ramps in, the bubble grows and drifts away from the focus so it never covers the zoomed area, with its own transition length and easing.
- Per-cut camera positioning. Set a different camera position for each cut and the bubble glides between them, with easing you control.
- Independent audio levels for the system and microphone tracks, each with its own volume, a mute toggle, and smooth fades.
- Loudness normalization on export, evening out overall volume to a broadcast target (EBU R128).
- Background music and extra audio clips on the timeline, with volume, looping, and fades.
- Title presets for dropping in a styled title in one click.
- A "dip" transition that briefly fades through the background on a scene change.

### Changed
- Rebuilt the preview's media pipeline. Frames are cached and reused instead of re-decoded, heavy work is deferred until the editor is idle, and a failed decode now recovers on its own rather than leaving the preview stuck on a stale frame.

## [0.4.2] — 2026-07-18

### Added
- Command-line control of projects and exports, on top of the existing recording verbs, so Recast can be driven from a script. Settings has a toggle for installing the `recast` command automatically.

### Changed
- The app now presents itself as "Recast" rather than "recast" in the window title, installer, and system metadata.
- Only one copy of Recast runs at a time. Opening a second one focuses the window you already have instead of starting a rival instance.
- The tray menu is reorganised, with your recent projects and exports listed directly in it.
- Refreshed the icon set across the sidebar and recording panel.

### Fixed
- On macOS, the FFmpeg binaries Recast downloads are cleared of the Apple quarantine flag. Without it macOS blocked them, and Recast silently fell back to whatever FFmpeg happened to be on your PATH, or to none at all.

## [0.4.1] — 2026-07-16

### Highlights
- Redesigned the editor's properties panel: sections now live in a compact, grouped vertical rail with a clear active state, and you can drag the panel wider.
- More precise timeline editing: the playhead tracks the video exactly, timecodes match everywhere, and the Cut tool works across every lane.

### Added
- A dedicated audio waveform lane in the timeline, shown alongside the thumbnails so you can cut against the sound.
- Drag across the Zoom or Cuts lane to create a zoom region or remove a section, the same gesture in both.
- A resizable properties panel: drag its edge to set the width, and it is remembered between sessions.
- Cut sections are now selectable and can be removed from the keyboard, like zoom regions and markup.
- A `transcribe` command for the command-line interface, so captions can be generated without opening the editor.

### Changed
- The properties panel section switcher is now a grouped vertical icon rail instead of a wrapping row of icons, with consistent active states and calmer motion.
- In the Background section, the controls follow the order you build a look: background first, then framing (padding and corner radius), then drop shadow. Corner radius has a tighter range and finer steps.
- Timeline editing shortcuts (Split, Cut, and set in/out points) now work without clicking the timeline first, and the on-screen keys reflect that.
- The smoother WebCodecs playback engine is now the default, so playback stays fluid across cuts and splits. It falls back to the standard player automatically where a device cannot use it.
- The editor now respects the system "reduce motion" setting throughout.
- The preview shows a scrubber only when the timeline is hidden, so there are no longer two scrubbers at once.

### Fixed
- The Cut (razor) tool now works when clicking over any lane, not only empty timeline space, and can be exited with the keyboard or Escape.
- Undo now correctly reverses a cut dragged out on the Cuts lane.
- The timeline playhead no longer lags behind the video during playback.
- Exports crashed on macOS when the hardware encoder was asked for a quality target it does not accept. It is now given a bitrate instead.
- Exporting burned-in captions with an FFmpeg build that lacks the subtitle filters now fails with a message that says so, instead of producing a video with no captions in it.
- The bundled FFmpeg is verified to ship those subtitle filters at build time, so that case should not reach you in the first place.

## [0.4.0] — 2026-07-11

### Highlights
- Control recording from any app with global shortcuts, and never lose a take to the screen going to sleep.
- Watch export progress on the Windows taskbar or macOS dock, and get a notification when it finishes.
- Right-click the Windows taskbar icon to start a new recording or reopen a recent project.
- An optional translucent window backdrop that matches Windows 11 and macOS.

### Added
- Global recording shortcuts. Start or stop a recording with Alt+Shift+R and pause or resume with Alt+Shift+P from any app, so you can control capture while Recast is in the background.
- A notification when an export finishes, plus live export progress on the Windows taskbar and the macOS dock.
- Pause and resume from the tray menu, which now also shows when a recording is in progress.
- Windows taskbar jump list. Right-click the taskbar or Start icon for a "New Recording" action and your recent projects.
- "Open in Recast" in the right-click menu for .recast files on Windows.
- Translucent window backdrop (Mica on Windows 11, vibrancy on macOS), off by default under Settings, Appearance.
- Redesigned captions with a compact, Loom-style look: a rounded translucent pill and word-by-word highlighting, where each word brightens as it is spoken and stays lit. It is the default in both the editor preview and the shared web player, and every built-in and extension-pack preset was modernized to match, with smooth, subtle entrance motion.
- New caption controls for the highlight mode, unspoken-word color, pill padding and corner radius, line height, and wrap width.
- The player loads a caption sidecar automatically: previewing an export finds a matching .vtt or .srt next to the file, and sharing a recording from the exports library uploads that sidecar as the caption track, so library shares carry captions even without an open project.

### Changed
- New recordings and exports save to a Recast folder in your Videos directory by default, instead of a temporary folder the system can clear. Change it anytime in Settings.
- Recording and exporting keep the display and system awake, so a long capture or export is never cut short by the screen or machine going to sleep.
- The Windows installer and update windows now carry the Recast icon and artwork.
- Refreshed the Google Drive connection page to match the app.
- On-device captions now work on Intel Macs, which the previous speech engine could not support. Apple Silicon, Windows, and Linux are unchanged.
- The player control bar auto-hides after a couple of seconds of pointer inactivity and fades back on movement. Captions stay visible on their own (they no longer fade with the controls) and bottom captions lift clear of the bar while it is showing so they never overlap. The captions button only appears when a track is available and now shows a clear on/off state.

### Fixed
- Double-clicking a .recast file opens it on macOS, which previously worked only on Windows and Linux.
- Burned-in captions now match the editor preview: correct size (they were rendering smaller because libass scales by the font's window metrics, not the em box, up to nearly half size for some display fonts), proper kerning, and the correct font at non-standard weights (a semibold previously fell back to a system face). Single-line captions get an exact rounded pill at export.

## [0.3.1] — 2026-07-05

### Highlights
- Scene animations: give any clip an entrance and exit (fade, slide, scale, pop, shrink, or rotate) that plays in the preview and renders in the exported video.
- Image annotations: drop a PNG or JPG onto the canvas with its own border, corner radius, and shadow, and it renders in the preview and the exported video.
- Annotations can pin to the video and track its zoom, or pin to the frame and hold still while the footage moves under them.
- Exports are roughly 3.5× faster. A 46-second recording that took 5m42s now finishes in about 1m37s.

### Added
- Scene animations. Each clip can animate into and out of view (fade, slide, scale, pop, shrink, or rotate) with full easing control per side. A project-wide motion tone (Subtle, Balanced, Energetic) tunes the intensity across the whole timeline, and a Push transition carries motion across a cut where content was removed. Animations play in the preview and render in the exported video.
- Image annotation tool. Import a PNG or JPG and it drops onto the canvas at its own aspect ratio, with a border, corner radius, and soft shadow. All of it renders in the preview and burns into the export, and a Replace action swaps the source in place.
- Per-annotation anchoring. Each annotation attaches to the Video, so it tracks zoom and focus, or to the Frame, so it stays put on the output while the footage moves under it. Works for shapes, text, and blur.
- In-app player for exported files. When an export finishes, play it right inside the editor instead of opening your file manager first.
- Option to hide the recording panel from screen captures (Settings). It applies immediately to a live recording. Windows and macOS support it; on Linux the setting explains why it can't yet.

### Changed
- Export defaults to 60fps for recordings above 60fps (Original, 30, and 24 stay selectable). It's imperceptible for a screen recording and roughly halves export time.
- The background image is blurred once at export instead of on every frame, which more than halved the encode time on its own.
- The export dialog names each prep step, rendering the cursor and annotation layer and then encoding, so it never sits on a blank "Preparing…".
- Redesigned the on-canvas annotation selection to match the recording area-select: real handles, a selection ring, and a live width-by-height badge. Hold Shift to lock aspect while resizing, snap a new shape to a square, or snap an arrow to 45 degrees. Moving an annotation snaps its own edges and center to the guides.
- New annotations take the current theme color instead of a fixed blue.
- Blur annotations can now be moved and duplicated, and they honor rounded corners in the export the same way the preview does. Corner radius across rectangles, blur, and images now uses a single 0–100% scale.
- Refreshed the Profiles page layout and accessibility.
- Copy cleanups across the editor panels, Settings, the sidebar, and the website.

### Fixed
- Scene animations now render in exported video, not just the preview.
- Export progress and the time-remaining estimate are measured against the real output length, so the bar no longer stalls short of or overshoots 100% on projects with cuts or speed changes.
- Annotation glow, blur, and images now render in the exported video with the same look as the preview (arrow glow stays preview-only).
- Text annotations wait for their font to load before rendering, so exported text no longer falls back to the wrong font. Text also survives a save and reload with all of its settings instead of reverting.
- A frame-anchored annotation keeps its anchor after you save and reopen a project instead of snapping back to the video.
- Scaled image annotations are smoothed instead of blocky, and a single corrupt annotation is skipped instead of failing the whole export.
- Export now warns, without blocking, when an image annotation can't be loaded or a blur sits under a zoom, since both would otherwise export silently wrong.

## [0.3.0] — 2026-07-01

### Highlights
- Animated captions: highlight the word being spoken, pop words in one at a time, or reveal short phrases.
- Captions sit around the padded video frame and export with their real fonts.

### Added
- Animated captions. Captions can now animate in time with speech: highlight the word being spoken karaoke-style, pop each word in one at a time, or fade in short phrases. A set of modern presets (Clean, Pill, Bold, Spotlight, Wave, Punch, Hype) ships with matching fonts, colors, and animation, and the theme picker previews each one. Controls for how words are grouped, the active-word highlight, the entrance effect, and position all live in the Captions tab.
- The Captions tab now tells you when a recording has no audio to transcribe, instead of letting you try and fail.

### Changed
- Captions are positioned against the whole video frame. When you add padding, top and bottom captions sit in the margin instead of overlapping the footage, and the preview matches the exported video. The position control moved next to alignment and now nudges captions in either direction over a wider range.
- Refreshed the built-in caption styles with modern fonts and colors.
- On-device captions need Apple Silicon, Windows, or Linux. On Intel Macs the Captions tab explains this and the rest of the editor works normally.

### Fixed
- Caption fonts now render in exported video, not just the preview.
- Exporting a recording that has cuts or speed changes no longer holds the last frame for several seconds or produces a file longer than the edit.

## [0.2.9] — 2026-06-29

### Added
- Update older recordings to the current project format in bulk from the library, or one at a time from a recording's menu. Recordings that still use the old format are marked so you can see which need updating.
- Hardware-accelerated encoding on Apple Silicon, so exports use the Mac's media engine instead of the CPU.

### Changed
- Silence detection is far more accurate. It now uses on-device voice detection to find genuinely silent, speech-free stretches and suggest them for removal, instead of judging by volume alone, which often mistook breathing, typing, and background hum for speech. Suggestions appear instantly when a recording opens, and now show up for camera-only recordings, not just screen recordings. Nothing is removed automatically, the quiet parts are only suggested for you to accept or skip.
- Recordings now save in a new project format that keeps each part of your edit (background, zoom, annotations, audio) in its own section, so project files are more robust and easier to inspect. Older recordings are updated to the new format when you open them, and a copy of the original is kept alongside it first.
- The editor stays responsive on long recordings. Adjusting cursor smoothing no longer freezes the preview, undo and redo are quicker, and autosave no longer causes a brief stutter.

### Fixed
- Recording a selected region on a Retina display captured the wrong area, because the selection was measured in points and the capture in pixels.
- Buttons went sticky and double-clicks were swallowed on macOS, caused by window drag regions overlapping the controls beneath them.

## [0.2.8] — 2026-06-28

### Highlights
- **Camera and microphone work on macOS again.** Fixed a permissions problem that stopped macOS from capturing your camera and mic.

### Changed
- Tightened copy across the desktop app and website. Removed em dashes and over-explanation from settings, experimental-feature, and editor-panel descriptions, and from a few marketing sections, so the text says what a feature does without explaining how it works internally.

### Fixed
- Website build no longer fails during prerender. A leftover static `robots.txt` was shadowing the dynamic `/robots.txt` route, so the route was never prerendered and the build errored out. The static file is gone; `/robots.txt` and `/sitemap.xml` now come from their routes.
- Corrected the Loom and Cap comparison on the Features page. Dropped a "pause and resume is paid in Loom" row (it is free), changed Cap's "share to your own storage" from "Not supported" to "Pro only" (Cap Pro supports custom S3 and Google Drive), and relabelled per-seat pricing.
- Fix camera and microphone permissions not working on macOS

## [0.2.7] — 2026-06-28

### Highlights
- **Editing feels faster on every new recording.** Recordings now capture a keyframe about twice a second, so seeking, scrubbing, and cutting no longer pause to re-decode several seconds of video.
- **Free in-browser video tools on the web.** Convert, trim, compress, and extract from a video right in your browser, with nothing uploaded.

### Added
- Free client-side video tools on the Recast website (`/tools`): MP4 to GIF, trim, mute, MP4 to MP3, extract audio, video to images, MOV to MP4, MP4 ↔ WebM, compress, and resize. Everything runs in the browser through WebCodecs. Files are never uploaded, and over-sized files are pointed at the desktop app, which has no limit. Each tool is its own page with a drag-and-drop upload, an input and output preview, and a download.
- Experimental WebCodecs preview engine improvements (Settings → Experimental → "WebCodecs preview"): playback now crosses cuts without freezing, audio stays sample-accurate across cuts via a new Web Audio engine, the decoded-frame cache sizes itself to the recording's resolution so 4K/5K clips don't stall the decoder, and very large recordings stream in over byte-ranges instead of loading the whole file into memory.

### Changed
- Recordings are encoded with a roughly half-second keyframe interval instead of the encoder default (about four seconds). Seeking, scrubbing, and crossing a cut in the editor are much quicker as a result, and export seeks speed up too, at a small increase in file size.

### Fixed
- Website SEO: removed duplicate page metadata (the default social card was leaking onto pages that set their own, so scrapers picked the generic one), added a sitemap and a proper robots.txt, marked non-public pages `noindex`, gave the privacy and terms pages their own canonical and social cards, and added site-wide brand structured data.

## [0.2.6] — 2026-06-10

### Highlights
- **Recording quality and frame rate are yours to set.** Capture at Balanced, High, or Pristine fidelity and pick a frame rate your display can actually deliver, instead of the previous fixed defaults.
- **Export frame rate is configurable too.** Keep the source rate or step down for a smaller file. A long-standing export "shake" on high-frame-rate clips is also fixed.
- **More extension packs.** New cursor, easing, smoothing, gradient, and wallpaper packs, and installed packs now appear right in the editor's preset pickers.

### Added
- Recording quality tiers (Balanced / High / Pristine) in Settings → Recording. Balanced reproduces the previous output exactly, so existing recordings are unchanged; High and Pristine trade real-time headroom for higher fidelity.
- Recording frame-rate selection (24–240 fps) in Settings → Recording, offering only the rates your monitor can produce based on its detected refresh rate. The chosen rate is now stored in the project, so high-refresh recordings are handled correctly throughout the editor and export.
- Export frame-rate control for MP4 and WebM: keep the original source rate (the default) or step down to a lower rate for a smaller file.
- New extension packs: Material and Windows 11 cursor styles, a cursor-smoothing preset pack, a motion-easing preset pack, a gradient collection, and a "Waves" wallpaper set.

### Changed
- Easing and smoothing preset pickers in the Cursor, Focus, and curve editors now read from the extension registry, so presets from installed packs appear alongside the built-ins instead of only the bundled set.
- The window titlebar moved to a full-width, OS-native bar above the sidebar and content, including left-aligned window controls on macOS for a more native feel.
- The export progress, success, cancelled, and error screens now share a consistent spec recap (format · quality · frame rate · duration) and width with the export options step.

### Fixed
- Exports of high-frame-rate recordings no longer judder or "shake". A generated background (solid colour, gradient, or image) defaulted FFmpeg to 25 fps and dragged the whole export down to it, frame-dropping 60 fps footage into juddery motion (most visible under a zoom). Generated backgrounds and looped image inputs are now pinned to the recording's frame rate.

## [0.2.5] — 2026-06-09

### Fixed
- Exported videos no longer open to a black screen stuck on "media loading" in the in-app player on release builds. The player now streams the file from the start instead of waiting on a tail fetch that never completed, so exports play back immediately.
- macOS and Linux: the app no longer freezes after a recording finishes. Saving a recording (flushing the encoder, finalizing the file, and the camera pause-trim re-encode) ran on the UI thread and locked the whole window until it completed. It now runs off the main thread. (Windows was unaffected because it renders the UI in a separate process.)
- macOS and Linux: starting a recording, listing recordings/exports, picking a microphone, and "reveal in file manager" no longer briefly freeze the window. These all moved off the UI thread for the same reason.
- Long recordings could freeze mid-capture: the encoder's FFmpeg progress output filled an OS pipe buffer that was never drained, stalling the encoder and the recording. Its output is now drained continuously.
- A recording that fails to start partway through no longer leaves orphaned capture/encoder processes running in the background.

## [0.2.4] — 2026-06-07

### Highlights
- **Extensions arrive.** Browse and install community asset packs (cursors, backgrounds, gradients, colours, and easing/smoothing presets) from a new Extensions tab. Packs are code-free and verified by HTTPS-only downloads with per-asset SHA-256 pinning.
- **Editor polishing pass continues.** The preset picker gains richer visual previews and predictable keyboard navigation, while the Info panel is reshaped into a more actionable, jump-to-tab summary.

### Added
- Extensions: browse and install community asset packs (cursors, backgrounds, gradients, colours, and easing/smoothing presets) from a new Extensions tab, with a local dev-registry server for authoring packs.

### Changed
- Preset picker refresh: the current preset is pinned and visibly marked, categories gain icons, wallpaper presets render real thumbnail previews, and arrow-key navigation now moves across the 2-column grid predictably instead of walking raw DOM order.
- Info panel redesign: source, project, and edit stats are reorganized into clearer cards with direct jump actions into the related editor tabs.

### Fixed
- Harden extension-pack installation: untrusted pack SVGs are rendered as images instead of inlined markup, asset paths and URL schemes are validated more strictly, and installed packs hydrate in a stable order.
- Development builds no longer send crash telemetry, so running the app locally never pollutes production analytics.

## [0.2.3] — 2026-06-06

### Highlights
- **Desktop diagnostics are now first-class.** A user-facing verbose-logging toggle plus log-management controls capture real diagnostic data on demand instead of asking users to reproduce issues blind.
- **Editor polish.** The audio panel was reshaped around segmented fade presets and a clearer control hierarchy, and a centralized keyboard-shortcut registry now powers a dedicated shortcuts dialog.

### Added
- Diagnostic logging controls in the desktop app: a feature flag / UI toggle for verbose logs, plus log management plumbing so debugging information can be turned on when needed instead of asking users to reproduce issues blind.
- Centralized keyboard-shortcut registry and a shortcuts dialog, with extra keyboard-event diagnostics in the desktop shell so modifier-key and stale-listener bugs can be traced from real `keydown` payloads when debugging editor shortcuts.

### Changed
- Audio panel redesign: fade presets move into a segmented control, output/fade controls get a clearer hierarchy, and the panel now states the shared system-audio + microphone mixing model more honestly.
- Desktop environment variables were consolidated so configuration reads from one clearer source of truth instead of drifting across multiple names and code paths.

### Fixed
- Tooltip positioning in the properties panel now avoids the previous clipping / overlap cases, making the labels readable around tighter panel layouts.

## [0.2.2] — 2026-06-05

### Highlights
- **Recast Cloud management got broader and sharper.** Uploads, shares, poster replacement, engagement tracking, and dashboard-side performance views all moved forward together.
- **Desktop playback and editing feel faster on real projects.** Thumbnail and waveform data can now be cached on disk instead of being recomputed every session.
- **Capture setup is more defensive.** Camera capability gating and browser-side device enumeration reduce bad hardware choices before recording starts.

### Added
- Poster replacement for recasts, plus engagement tracking and supporting shares / performance surfaces on the dashboard so cloud-hosted recordings are easier to manage after upload.
- Browser-side device enumeration and capability checks for cameras, helping the recorder present more reliable hardware choices before capture begins.
- New SVG cursor sprites and the supporting cursor-style management refactor, laying cleaner groundwork for richer cursor overlays in the editor and exports.

### Changed
- Dashboard upload and recast-management flows were expanded, giving Recast Cloud a more complete post-upload management surface instead of treating upload as the end of the workflow.
- Legacy share-visibility values are normalized more consistently, and share access management is clearer across older and newer recast records.
- Desktop environment configuration was reorganized, and the macOS capture path received follow-up handling improvements as the beta setup hardened.
- macOS installation guidance was tightened up so download and setup steps are clearer for beta users.

### Fixed
- Thumbnails and waveform data can now be cached to disk, cutting down repeated processing and improving responsiveness when reopening projects.

## [0.2.1] — 2026-06-03

### Highlights
- **Recast Cloud arrived in earnest.** Uploads, share links, password protection, expiry, workspace-aware routing, and self-host configuration all landed across web and desktop.
- **Library organization got real tools.** Tags, archives, and tag-management UI make larger recast collections manageable instead of flat lists.
- **Recording startup became more controllable.** Countdown support, per-profile delay overrides, and Windows aspect-ratio locking smooth out capture setup.

### Added
- Recast Cloud upload and share flows across the app, including workspace-aware upload routing and broader share-management plumbing for cloud-hosted recasts.
- Password-protected and expiring share links, plus account-less access for selected shares so private distribution has more than one mode.
- Tags and archives for recasts: API support, archived recast management, and a tag-management dialog for renaming, recoloring, and deleting tags.
- Self-hosting endpoint configuration in desktop settings through the `CloudEndpoint` settings surface.
- Recording countdown support with customizable duration and per-profile overrides, so different recording setups can start with different delays.
- Analytics groundwork across web and desktop for measuring product and sharing behavior as Recast Cloud rolls out.

### Changed
- Local desktop persistence moved from raw `localStorage` usage to `safeStorage`-backed handling where appropriate, improving resilience and synchronization for saved state.
- Azure storage configuration validation was hardened with constant-time comparison, reducing opportunities for subtle auth and config mistakes.
- Shared recast pages picked up release-process and SEO improvements, and the pricing table layout was adjusted to hold up better at narrower widths.
- The Windows recording window now respects aspect-ratio locking while resizing, making capture setup less fussy.

## [0.2.0] — 2026-05-30

### Highlights
- A single **morphing export dialog** that flows Options → Encoding → Success / Cancelled / Error without ever closing. Width and height ease between phases, and content cross-fades on top.
- **Sliding tab indicator** behind every `Tabs.List` (Settings, properties panel, source select): the active pill slides between tabs instead of snapping.
- Export Options redesigned end-to-end against `DESIGN.md`: GIF extras open as a smooth side panel on wide screens, fall back to an inline accordion on narrow ones, and the dialog auto-morphs its width as you switch formats.

### Added
- `ExportFlowDialog` wrapper component that owns the dialog chrome (portal, backdrop, scale-in, focus + Esc routing) and auto-morphs its width and height to whatever the active phase declares via a `ResizeObserver`. A custom out-transition absolute-positions the leaving phase so its fade-out can't drag the wrapper size around. The new phase mounts in normal flow, the wrapper Tweens to match, and the old phase fades on top.
- Per-phase Esc and backdrop routing: Esc cancels a running export, dismisses a finished one, or closes the options picker; the backdrop never cancels an in-flight encode (too easy to misclick mid-render).
- Share button on the export success card (when `navigator.share` is available), with sensible fallback messaging when the platform doesn't support sharing files but a Drive link is on hand.
- Sliding active-tab indicator inside `Tabs.List` (shared `@recast/ui` component). Driven by a Svelte 5 `Tween` plus a `MutationObserver` watching `data-state` changes, so it stays decoupled from `bits-ui` internals. Variant-aware visual: `soft` uses `bg-card + shadow-craft-inset`, `default` uses `bg-background + shadow-sm`, `line` slides a 2 px `bg-foreground` bar. Works in both horizontal and vertical orientations and snaps on first measure so it doesn't grow from `(0,0)`.

### Changed
- Export UI consolidated into one surface across three previously-separate states (options dialog, inline progress overlay, inline result overlay). This eliminates the close/reopen flash between picking a format and seeing encode progress, and again between encode finishing and the success card.
- Export Options dialog redesigned against `DESIGN.md` dialog rhythm: header `px-5 py-4` with title + description, section dividers softened to `border-border/40`, footer `bg-muted/30 py-2.5`, stat strip inlined with a single divider instead of nested glass cards, section labels paired with a one-line description per the design vocabulary. Buttons use the canonical glass surface (`bg-card/40 + border-border/40`) with `bg-primary/8 + ring-primary/25` for selection.
- GIF extras (frame rate, color richness, gradients, loop) now reveal as a side panel on wide screens (the dialog grows from 440 px to 760 px through the flow dialog's morph rather than animating an internal collapse) and stack as an inline accordion when the viewport is narrower than 720 px.
- Export Options dialog is now responsive: container clamps to `min(820px, calc(100vw - 2rem))` and the body picks its own natural width that the wrapper auto-morphs to.
- `EditorToolbar` no longer mounts its own `ExportDialog`; the toolbar's Export button now bubbles a single `onexport` callback up to the editor page, which owns the flow phase.
- Progress, Success, Cancelled, and Error views adopted the same chrome and spacing rhythm as the Options view: `size-10 rounded-xl` status icon badges, consistent footer padding, primary actions on the right.

### Fixed
- No more visual "snap" when switching the export format between MP4/WebM and GIF. The GIF settings panel mounts inline and the wrapper morphs to the new natural size in one motion.
- Focus is re-routed back into the dialog on every phase change, so screen readers re-announce and keyboard navigation stays inside the modal as content swaps under the user.

## [0.1.10] — 2026-05-28

### Highlights
- **Google Drive uploads** straight from the export success card, with per-upload progress, history, and cancel/retry. The first "send it somewhere" target after local files.
- **Account and authentication** across desktop and web: device-authorization OAuth flow on the app, magic-link + password sign-in on the web, plus a templated transactional-email system behind both.
- **Hardware-accelerated exports** on NVIDIA / AMD / Intel where available, with startup probing so the app picks the right encoder once and remembers. Multi-threaded VP9 and camera pause-trim land on the recording path too.
- **macOS feature parity work**: native `ScreenCaptureKit` audio loopback, cross-platform cursor sampling, and the macOS / Linux audio + camera platform modules wired through the recorder.
- **Tabbed Settings** layout (General / Local / Cloud) and a **frame snapshot → clipboard** action in the editor.

### Added
- Google Drive integration: connect from Settings → Cloud, upload exports from the success card, watch live upload progress with a per-upload progress bar, cancel in flight, retry failures, copy or open the Drive link once it's done, and review a per-file upload history that survives dismissals.
- OAuth 2.0 Device Authorization Grant flow for the desktop app, with the matching UI components (device code display, polling state, success card), so the app can sign in without ever embedding a browser window.
- Magic-link sign-in and password-reset on the web, backed by Better Auth + Drizzle, with templated transactional emails (layout + transport abstraction so future templates plug in cleanly).
- Cross-window panel error routing through sonner toasts. Rust-side errors from the recording panel now surface as proper toasts in the main window instead of vanishing into the panel's own console.
- Admin surface for the web: user management, waitlist approvals, teams management, and impersonation with transaction-safe team creation / switching.
- `NavProgress` component for a top-of-page navigation indicator, with a generation token so stale completion callbacks from cancelled navigations can't flash the bar.
- macOS-only `ScreenCaptureKit` audio loopback gated behind an opt-in `sckit-loopback` feature flag, and a cross-platform cursor sampler that finally unblocks the macOS / Linux recording paths.
- Hardware-encoder startup probe + documentation of hardware requirements, so the encoder picker no longer fails late inside FFmpeg when a GPU encoder isn't actually installed.
- Tabbed Settings interface (Local / Cloud / General) replacing the previous single-column scroll, with each tab keeping its own subtle slide-in.
- Editor "capture frame" action: grab the current composited frame and copy it to the clipboard from the player controls.
- Homebrew Cask publishing workflow and matching install instructions for macOS alongside the existing `.dmg`, `.deb`, `.AppImage`, and `.exe` artifacts.
- Pricing page footer / navbar "Join Waitlist" entry and a refreshed pricing layout.
- Top-level formatting + linting scripts wired through Turbo, so `pnpm format` and `pnpm lint` run consistently across the monorepo.

### Changed
- Export pipeline now multi-threads VP9 encodes and hardware-accelerates AMD / Intel paths in addition to NVENC, with a RAM-bounded capture queue to prevent runaway memory during long recordings.
- Editor performance: thumbnails are batched into a single FFmpeg call, the preview falls back to WebGL2 where supported, and a temp-file sweep reclaims scratch storage during sessions.
- Camera pause-trim is now hardware-accelerated end-to-end, removing the worst stalls on long captures with camera overlay.
- Smart-zoom suggestions tightened with improved scoring + clustering (continuing the 0.1.8 rework with better dedupe behavior under repeat clicks).
- Toaster + theming updated for consistent visual language across the corner notifications it shares space with.
- Trusted-origins handling in `better-auth` now reads CSV-formatted env vars, and the env schema defaults sensible URLs for optional CSV fields so first-run setups don't trip on missing values.

### Fixed
- Updater manifest generation now runs even when one of the per-platform build legs fails, so a partial release no longer leaves the auto-updater pointing at the previous version forever.
- MSIX builds now stage the FFmpeg sidecars correctly (and stop uploading internal `.deb` payloads as release artifacts).
- FFmpeg / ffprobe spawn audit completed: every spawn site uses `configure_silent_command` on Windows, so console-flash focus theft no longer reads as "the whole window froze".
- "Recording stop" failures no longer get blamed on FFmpeg by default. The UI now resets client-side state cleanly on stop-failure and reports the actual cause when there is one.
- Diagnostics: file logging stays enabled in release builds and surfaces the full `anyhow` cause chain, so support reports actually contain the root error.
- Pinned `apple-metal` to `0.6.1` for CI compatibility so macOS leg builds don't break on transitive bumps.
- Contact email updated to the new address in Footer and Navbar.
- Various button + UI fixes: prevent text selection on `<Button>`, button hover regressions, and a Vercel deploy workflow tweak so install no longer fails on lockfile drift.

## [0.1.9] — 2026-05-23

### Added
- Inline playback for recordings: tapping a card on the exports page now
  opens a `PlayerDialog` powered by `@recast/player` (RecastPlayer) with the
  branded media-chrome controls, instead of jumping straight to the file
  location. "Show in folder" stays one click away inside the dialog footer.
- Global `@recast/player/styles.css` import in the desktop root layout so
  any future inline players pick up the same theming without per-route
  boilerplate.

### Fixed
- Pointer-events leak from floating UI surfaces in the Tauri build:
  `DropdownMenu`, `HoverCard`, `Popover`, and `Select` content wrappers now
  also default `preventScroll={false}` (matching the earlier `Dialog` and
  `Sheet` fix from 0.1.6), so a closed menu or popover can no longer leave
  `pointer-events: none` on the document body and freeze the window.

## [0.1.8] — 2026-05-22

### Added
- Pause and resume during recording with controls in the recording panel and
  a clearer status indicator, so a notification or knock at the door no longer
  forces a restart.
- Auto-updater and "What's new" notifications in the bottom-right corner of
  the editor, so release prompts and changelog nudges stay out of the way of
  the timeline.
- Silence detection (phase 1, opt-in under Settings → Experimental): finds
  dead-air segments by combining waveform analysis with cursor idleness, then
  offers one-click cuts you can review or dismiss.
- Dashboard route with a local-storage-backed data layer for recordings and
  exports, plus first analytics hooks.
- Web auth foundation: magic-link sign-in and password-reset flows backed by
  Better Auth + Drizzle, plus a public waitlist endpoint for Recast Cloud.
- macOS and Linux platform modules for audio and camera capture, paving the
  way for full feature parity with the Windows build.
- Homebrew Cask publishing workflow and matching install instructions for
  macOS alongside the existing `.dmg`, `.deb`, `.AppImage`, and `.exe`
  artifacts.

### Changed
- Smart-zoom suggestions: new scoring model that clusters clicks, weighs
  dwell time, and dedupes same-spot triggers, so auto-applied focus regions
  land on the moments that actually matter instead of every mouse-down.
- Toaster restyled to share visual language with the bottom-right update
  notifications: same card geometry, same close affordance, same icon-badge
  variants. Sits in `bottom-right` everywhere instead of `top-center`.
- Marketing site: hero copy rewritten to honestly describe the timeline
  ("the lightest editor you've used") instead of pretending it doesn't
  exist; new editor-tour rail showcases the auto and manual tools side by
  side. Features, gamers, pricing pages refreshed too.
- Recordings library cards (web + desktop) picked up techy framing
  (dot-grid placeholders, primary glow, CRT-style corner brackets) so an
  empty thumbnail reads as "ready for a frame" instead of an empty hole.

### Fixed
- Window-freeze regression on recording start: every FFmpeg/ffprobe spawn
  site now uses `configure_silent_command` on Windows so the console flash
  no longer steals focus and reads as "the whole window is frozen".
- Closing the recorder window while a recording is in flight no longer
  drops the capture; the app prompts and resolves the save first.

## [0.1.7] — 2026-05-16

### Added
- Bulk-select mode for recordings and exports, with a floating action bar
  for delete and a single-tap "select all".
- Morph animations when toggling between grid and list views on the
  recordings and exports pages, with no jarring re-flow.
- One-shot setup scripts (`setup.ps1` / `setup.sh`) so first-time
  contributors can bring the whole monorepo up with a single command on
  Windows or macOS/Linux.

### Changed
- Export filenames now suffix duplicates with `(1)`, `(2)`, ... via a
  shared `unique_path` helper, so re-exporting the same recording keeps
  both files instead of silently overwriting the previous one.
- Quick-start docs screenshot refreshed to show region selection.

### Fixed
- Hero CTA region: removed an unused background layer that was painting a
  stray gradient behind the headline on some viewport widths.

## [0.1.6] — 2026-05-10

### Added
- Version-sync release scripts: every build manifest validates against the
  release tag and fails fast if a `0.0.0-dev` placeholder slips through.
- GitHub issue templates for bug reports, feature requests, and
  performance issues.

### Changed
- Dialog and Sheet components default `preventScroll={false}` so a closed
  dialog can no longer leak `pointer-events: none` onto the document body
  inside Tauri. This was the root cause of the earlier "the whole window
  is dead" reports.

### Fixed
- Resolved an intermittent pointer-blockage bug in the Dialog component
  that froze interactions after closing a modal.
- Version placeholders unified across files so dev and release builds no
  longer disagree about who they are.

## [0.1.5] — 2026-05-09

### Added
- Linux screen capture: a Wayland-native pipeline using
  `xdg-desktop-portal` + PipeWire, and a parallel X11 native capture
  path. Linux recording docs refreshed alongside the new backends.
- Recording profiles: per-launch capture profiles with dynamic capability
  combinations, device awareness, and a management UI in Settings.
- Command palette (⌘K) extracted into a global `CommandPaletteHost`
  mounted at the root layout, so the shortcut and dialog work on every
  route (including the editor), not only on routes that render the
  sidebar.
- Web download page redesigned with new platform icons and a feature
  grid.

### Changed
- Properties panel: shared `PanelSection` primitive replaces ~30 ad-hoc
  section headers, drops repeated panel-name titles, normalises gap to
  `gap-4`, and standardises toggle / reset placement across Background,
  Focus, Annotations, Cursor, Audio, Camera, and Info panels.
- Design tokens: introduced a Framer-inspired vocabulary (`canvas`,
  `surface-1/2`, `ink`, `ink-muted`, `hairline`, gradient spotlight cards,
  elevation shadows) layered on top of the existing shadcn tokens.
  Primary colour and font stack preserved.

## [0.1.4] — 2026-05-08

### Added
- Camera overlay in the editor: composite the recorded camera track over
  the screen video with position presets, size, shape, and mirror
  toggles. Gated behind a `CAMERA_OVERLAY_UI_ENABLED` feature flag.
- Cursor: mouse-press events feed into the recorded timeline, and a
  refreshed set of cursor styles ships with the editor.
- Native macOS-style page transitions via the View Transitions API, with
  a smoother titlebar handoff between routes.

### Changed
- Canvas geometry and aspect-ratio handling: editor geometry helpers now
  carry the chosen aspect end-to-end (preview, composite, drop-shadow)
  without per-call ad-hoc math.

## [0.1.3-beta] — 2026-05-07

### Added
- Active-preset chip in the editor toolbar with a reset-to-source affordance.
- Per-project preset persistence: applied preset and output aspect round-trip
  with undo/redo and project autosave.

### Changed
- GIF export now uses a 2-pass palettegen → paletteuse pipeline, so the
  progress bar advances in real time instead of sitting at 0% while only the
  elapsed counter ticked.
- Presets actually resize the canvas to their target aspect (16:9, 9:16,
  1:1, 1.91:1) end-to-end through the preview, FFmpeg filter graph, cursor
  overlay, and drop-shadow rasteriser.
- Stronger blur annotation: redacts content even at full strength, with
  scaled tint opacity and an optional gray wash above 0.6 strength.
- FFmpeg error reporting filters out progress noise so real diagnostic
  lines reach the failure toast.

### Fixed
- Region picker "Use area" / "Cancel" buttons now work; closing the main
  window exits the app instead of leaving aux windows holding the process.
- Quick action no longer opens the camera preview inside the recording
  panel window.

## [0.1.2-beta] — 2026-05-06

### Added
- Timeline workspace: clip bar, playhead, ruler, toolbar, and zoom lane components.
- Blur annotations with adjustable strength, rendered through the composite canvas pipeline.
- Cursor animation effects: click bounce, idle sway, and motion blur.
- Glass card and chip components for a more refined UI surface.
- `Kbd` component for consistent keyboard shortcut hints.
- Region selection in the source picker, with last-used source persistence.
- Camera overlay settings and validation, plus browser-based camera enumeration.
- Command palette (⌘K) with global navigation, recording, theme and external commands.
- Sidebar pinning and hover behavior.

### Changed
- Refactored project structure for readability and maintainability.
- Upgraded Node.js to v24 and enabled `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24`.
- Redesigned loading screen with new logo and progress bar.
- Polished typography, spacing, and accessibility across annotation panels and headers.

### Fixed
- Reverted erroneous app version bump; settings layout regressions cleaned up.

## [0.1.0-beta] — Initial beta

- First public beta of Recast: offline-first desktop screen recorder and editor
  built on Tauri v2, Svelte 5, and Rust.
