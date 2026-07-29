export {
	type Ctx2D,
	type StrokeStyle,
	type Point,
	type ArrowGeometry,
	strokeDashPattern,
	blurTint,
	withAlpha,
	arrowGeometry,
	roundRectPath,
} from "./draw-primitives";
export {
	type Rect,
	type ZoomTransform,
	type UVPoint,
	type NormalisableKind,
	uvToCanvas,
	canvasToUV,
	normaliseBox,
} from "./projection";
export {
	type RenderStroke,
	type RenderGlow,
	type RenderableAnnotation,
	type ShapeImage,
	type ShapeDeps,
	paintArrow,
	paintBoxAnnotation,
} from "./annotation-render";
