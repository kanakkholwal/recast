export type ColorValue = string;

function clamp01(v: number): number {
	return Math.max(0, Math.min(1, v));
}

export function parseColor(input: string): { r: number; g: number; b: number; a: number } | null {
	const v = input.trim();
	if (!v) return null;
	// #rgb / #rgba
	let m = v.match(/^#([0-9a-f])([0-9a-f])([0-9a-f])([0-9a-f])?$/i);
	if (m) {
		const r = parseInt(m[1] + m[1], 16);
		const g = parseInt(m[2] + m[2], 16);
		const b = parseInt(m[3] + m[3], 16);
		const a = m[4] ? parseInt(m[4] + m[4], 16) / 255 : 1;
		return { r, g, b, a };
	}
	// #rrggbb / #rrggbbaa
	m = v.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})?$/i);
	if (m) {
		const r = parseInt(m[1], 16);
		const g = parseInt(m[2], 16);
		const b = parseInt(m[3], 16);
		const a = m[4] ? parseInt(m[4], 16) / 255 : 1;
		return { r, g, b, a };
	}
	// rgb(...) / rgba(...)
	m = v.match(/^rgba?\(([^)]+)\)$/i);
	if (m) {
		const parts = m[1]
			.split(/[,\s]+/)
			.filter(Boolean)
			.map((p) => p.trim());
		if (parts.length < 3) return null;
		const r = Math.round(Number(parts[0]));
		const g = Math.round(Number(parts[1]));
		const b = Math.round(Number(parts[2]));
		const a = parts[3] !== undefined ? clamp01(Number(parts[3])) : 1;
		if ([r, g, b].some(Number.isNaN)) return null;
		return { r, g, b, a };
	}
	return null;
}

export function formatHex(r: number, g: number, b: number, a: number): string {
	const rh = r.toString(16).padStart(2, "0");
	const gh = g.toString(16).padStart(2, "0");
	const bh = b.toString(16).padStart(2, "0");
	if (a >= 1) return `#${rh}${gh}${bh}`;
	const ah = Math.round(clamp01(a) * 255)
		.toString(16)
		.padStart(2, "0");
	return `#${rh}${gh}${bh}${ah}`;
}

export function rgbToHsl(
	r: number,
	g: number,
	b: number,
): { h: number; s: number; l: number; a: number } {
	const rn = r / 255;
	const gn = g / 255;
	const bn = b / 255;
	const max = Math.max(rn, gn, bn);
	const min = Math.min(rn, gn, bn);
	const l = (max + min) / 2;
	let h = 0;
	let s = 0;
	if (max !== min) {
		const d = max - min;
		s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
		switch (max) {
			case rn:
				h = (gn - bn) / d + (gn < bn ? 6 : 0);
				break;
			case gn:
				h = (bn - rn) / d + 2;
				break;
			case bn:
				h = (rn - gn) / d + 4;
				break;
		}
		h *= 60;
	}
	return { h: Math.round(h), s: Math.round(s * 100), l: Math.round(l * 100), a: 1 };
}

export function hslToRgb(h: number, s: number, l: number): { r: number; g: number; b: number } {
	const hn = h / 360;
	const sn = s / 100;
	const ln = l / 100;
	if (sn === 0) {
		const v = Math.round(ln * 255);
		return { r: v, g: v, b: v };
	}
	const q = ln < 0.5 ? ln * (1 + sn) : ln + sn - ln * sn;
	const p = 2 * ln - q;
	const hueToRgb = (t: number) => {
		let tn = t;
		if (tn < 0) tn += 1;
		if (tn > 1) tn -= 1;
		if (tn < 1 / 6) return p + (q - p) * 6 * tn;
		if (tn < 1 / 2) return q;
		if (tn < 2 / 3) return p + (q - p) * (2 / 3 - tn) * 6;
		return p;
	};
	return {
		r: Math.round(hueToRgb(hn + 1 / 3) * 255),
		g: Math.round(hueToRgb(hn) * 255),
		b: Math.round(hueToRgb(hn - 1 / 3) * 255),
	};
}
