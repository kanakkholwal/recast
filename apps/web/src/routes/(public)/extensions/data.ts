import {
    Blend,
    Blocks,
    Captions,
    Check,
    Download,
    Hash,
    Image as ImageIcon,
    Layers,
    Lock,
    MousePointer2,
    Palette,
    ShieldCheck,
    Spline,
    Waves
} from "@lucide/svelte";

// The contribution kinds a pack can add. These map 1:1 to the editor's
// pickers, so the page reads as "this shows up where you already work".
export const kinds = [
    { icon: MousePointer2, title: "Cursors", description: "New pointer styles with rest and click states, plus precise hotspots, right in the cursor picker." },
    { icon: ImageIcon, title: "Backgrounds", description: "Wallpapers that drop straight into the canvas background picker." },
    { icon: Blend, title: "Gradients", description: "Curated gradient sets, rendered live in both the preview and the export." },
    { icon: Palette, title: "Colors", description: "Solid color swatches for the canvas, ready to click." },
    { icon: Captions, title: "Caption themes", description: "Ready-made caption looks (font, color, outline, backing), applied in one click to overlay and burned-in captions." },
    { icon: Spline, title: "Easing presets", description: "Named motion curves for zoom and cursor animation." },
    { icon: Waves, title: "Smoothing presets", description: "Cursor smoothing recipes, strength plus click snap, that you can share as packs." },
];


export const steps = [
    { icon: Blocks, title: "Browse or paste", description: "Open Extensions, browse the registry, or paste a pack URL to install directly." },
    { icon: Download, title: "Install in a click", description: "Downloads, every asset gets hash-checked, installs locally. No account, nothing phones home." },
    { icon: Layers, title: "Use it everywhere", description: "Cursors, backgrounds, gradients, presets show up in the pickers you already use." },
];

export const trust = [
    { icon: Lock, title: "No code runs", description: "A pack is a manifest plus static files. Nothing executes." },
    { icon: Hash, title: "Hash-pinned over HTTPS", description: "Every asset is checked against its SHA-256 on download. Tampered files fail the install." },
    { icon: ShieldCheck, title: "Zero permissions", description: "Asset packs can't request capabilities. A pack never reaches further than the app already can." },
    { icon: Check, title: "Open and checked", description: "The registry is public on GitHub, CI checks every submission for schema, hashes, and safe filenames." },
];
