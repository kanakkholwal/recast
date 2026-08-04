/**
 * Minimal WebGL2 shader/program helpers. Pure w.r.t. the component: every call
 * takes the GL context as its first argument, so these can be unit-tested with a
 * mock context and shared across compositors.
 */

export function compile(g: WebGL2RenderingContext, type: number, src: string): WebGLShader {
	const sh = g.createShader(type)!;
	g.shaderSource(sh, src);
	g.compileShader(sh);
	if (!g.getShaderParameter(sh, g.COMPILE_STATUS)) {
		const log = g.getShaderInfoLog(sh);
		g.deleteShader(sh);
		throw new Error(`Shader compile failed: ${log}`);
	}
	return sh;
}

export function link(g: WebGL2RenderingContext, vs: WebGLShader, fs: WebGLShader): WebGLProgram {
	const p = g.createProgram()!;
	g.attachShader(p, vs);
	g.attachShader(p, fs);
	g.linkProgram(p);
	if (!g.getProgramParameter(p, g.LINK_STATUS)) {
		const log = g.getProgramInfoLog(p);
		g.deleteProgram(p);
		throw new Error(`Program link failed: ${log}`);
	}
	return p;
}
