/** Full background + aspect data transcribed verbatim from screenshot-studio
 * (lib/constants/{gradient-colors,mesh-gradients,solid-colors,aspect-ratios}.ts).
 * Generated to keep the port's picker at parity with upstream counts. Do not
 * hand-edit; regenerate from the reference if upstream changes. */
import type { AspectPreset, BackgroundPreset } from "./types";

function grad(id: string, label: string, css: string): BackgroundPreset {
	return { id, label, background: { kind: "gradient", css }, swatch: css };
}
function solid(id: string, label: string, color: string): BackgroundPreset {
	return { id, label, background: { kind: "solid", color }, swatch: color };
}

/** 102 classic linear gradients. */
export const REF_GRADIENTS: BackgroundPreset[] = [
	grad(
		"grad-vibrant_orange_pink",
		"Vibrant Orange Pink",
		"linear-gradient(135deg, rgb(255, 100, 50) 12.8%, rgb(255, 0, 101) 43.52%, rgb(123, 46, 255) 84.34%)",
	),
	grad(
		"grad-peach_pink_purple",
		"Peach Pink Purple",
		"linear-gradient(135deg, rgb(255, 177, 122) 12.8%, rgb(233, 107, 189) 43.52%, rgb(123, 79, 255) 84.34%)",
	),
	grad(
		"grad-cyan_blue_purple",
		"Cyan Blue Purple",
		"linear-gradient(135deg, rgb(0, 255, 229) 12.8%, rgb(75, 108, 255) 43.52%, rgb(156, 31, 217) 84.34%)",
	),
	grad(
		"grad-orange_pink_dark",
		"Orange Pink Dark",
		"linear-gradient(135deg, rgb(255, 184, 107) 12.8%, rgb(255, 69, 133) 43.52%, rgb(47, 28, 150) 84.34%)",
	),
	grad(
		"grad-green_teal_navy",
		"Green Teal Navy",
		"linear-gradient(135deg, rgb(71, 246, 132) 12.8%, rgb(0, 184, 169) 43.52%, rgb(24, 78, 104) 84.34%)",
	),
	grad(
		"grad-pink_red_yellow",
		"Pink Red Yellow",
		"linear-gradient(135deg, rgb(255, 97, 230) 12.8%, rgb(255, 51, 51) 43.52%, rgb(255, 184, 0) 84.34%)",
	),
	grad(
		"grad-cyan_blue_violet",
		"Cyan Blue Violet",
		"linear-gradient(135deg, rgb(0, 255, 224) 12.8%, rgb(0, 102, 255) 43.52%, rgb(102, 0, 255) 84.34%)",
	),
	grad(
		"grad-peach_pink_lavender",
		"Peach Pink Lavender",
		"linear-gradient(135deg, rgb(255, 177, 122) 12.8%, rgb(255, 77, 109) 43.52%, rgb(132, 94, 194) 84.34%)",
	),
	grad(
		"grad-lime_cyan_blue",
		"Lime Cyan Blue",
		"linear-gradient(135deg, rgb(89, 255, 0) 12.8%, rgb(0, 255, 209) 43.52%, rgb(0, 102, 255) 84.34%)",
	),
	grad(
		"grad-pink_red_burgundy",
		"Pink Red Burgundy",
		"linear-gradient(135deg, rgb(255, 94, 154) 12.8%, rgb(255, 0, 61) 43.52%, rgb(144, 0, 72) 84.34%)",
	),
	grad(
		"grad-blue_pink",
		"Blue Pink",
		"linear-gradient(135deg, rgb(52, 148, 230), rgb(236, 110, 173))",
	),
	grad(
		"grad-green_blue",
		"Green Blue",
		"linear-gradient(135deg, rgb(103, 178, 111), rgb(76, 162, 205))",
	),
	grad(
		"grad-pink_orange",
		"Pink Orange",
		"linear-gradient(135deg, rgb(238, 9, 121), rgb(255, 106, 0))",
	),
	grad(
		"grad-teal_navy",
		"Teal Navy",
		"linear-gradient(135deg, rgb(67, 198, 172), rgb(25, 22, 84))",
	),
	grad(
		"grad-pink_burgundy",
		"Pink Burgundy",
		"linear-gradient(135deg, rgb(243, 98, 101), rgb(150, 18, 118))",
	),
	grad(
		"grad-blue_lavender",
		"Blue Lavender",
		"linear-gradient(135deg, rgb(97, 144, 232), rgb(167, 191, 232))",
	),
	grad(
		"grad-green_navy",
		"Green Navy",
		"linear-gradient(135deg, rgb(52, 232, 158), rgb(15, 52, 67))",
	),
	grad(
		"grad-lime_mint",
		"Lime Mint",
		"linear-gradient(135deg, rgb(170, 255, 169), rgb(17, 255, 189))",
	),
	grad(
		"grad-cyan_blue",
		"Cyan Blue",
		"linear-gradient(135deg, rgb(178, 254, 250), rgb(14, 210, 247))",
	),
	grad(
		"grad-mint_sky",
		"Mint Sky",
		"linear-gradient(135deg, rgb(132, 250, 176), rgb(143, 211, 244))",
	),
	grad(
		"grad-lavender_pink",
		"Lavender Pink",
		"linear-gradient(135deg, rgb(166, 192, 254), rgb(246, 128, 132))",
	),
	grad(
		"grad-purple_sky",
		"Purple Sky",
		"linear-gradient(135deg, rgb(224, 195, 252), rgb(142, 197, 252))",
	),
	grad(
		"grad-cyan_lime",
		"Cyan Lime",
		"linear-gradient(135deg, rgb(0, 201, 255), rgb(146, 254, 157))",
	),
	grad(
		"grad-navy_blue",
		"Navy Blue",
		"linear-gradient(135deg, rgb(63, 43, 150), rgb(168, 192, 255))",
	),
	grad(
		"grad-blue_teal",
		"Blue Teal",
		"linear-gradient(135deg, rgb(0, 147, 233), rgb(128, 208, 199))",
	),
	grad(
		"grad-mint_yellow",
		"Mint Yellow",
		"linear-gradient(135deg, rgb(133, 255, 189), rgb(255, 251, 125))",
	),
	grad(
		"grad-sky_blue",
		"Sky Blue",
		"linear-gradient(135deg, rgb(171, 220, 255), rgb(3, 150, 255))",
	),
	grad(
		"grad-green_cyan",
		"Green Cyan",
		"linear-gradient(135deg, rgb(81, 207, 102), rgb(55, 213, 214))",
	),
	grad(
		"grad-pink_purple_blue",
		"Pink Purple Blue",
		"linear-gradient(135deg, rgb(255, 60, 172), rgb(120, 75, 160), rgb(43, 134, 197))",
	),
	grad(
		"grad-pink_cyan_lime",
		"Pink Cyan Lime",
		"linear-gradient(135deg, rgb(250, 139, 255), rgb(43, 210, 255), rgb(43, 255, 136))",
	),
	grad(
		"grad-blue_purple",
		"Blue Purple",
		"linear-gradient(135deg, rgb(139, 198, 236), rgb(149, 153, 226))",
	),
	grad(
		"grad-mint_pink",
		"Mint Pink",
		"linear-gradient(135deg, rgb(62, 236, 172), rgb(238, 116, 225))",
	),
	grad(
		"grad-blue_pink_dark",
		"Blue Pink Dark",
		"linear-gradient(135deg, rgb(2, 80, 197), rgb(212, 63, 141))",
	),
	grad(
		"grad-pink_blue",
		"Pink Blue",
		"linear-gradient(135deg, rgb(252, 70, 107), rgb(63, 94, 251))",
	),
	grad(
		"grad-purple_pink_white",
		"Purple Pink White",
		"linear-gradient(135deg, rgb(115, 3, 192), rgb(236, 56, 188), rgb(253, 239, 249))",
	),
	grad(
		"grad-blue_pink_yellow",
		"Blue Pink Yellow",
		"linear-gradient(135deg, rgb(65, 88, 208), rgb(200, 80, 192), rgb(255, 204, 112))",
	),
	grad(
		"grad-cyan_magenta",
		"Cyan Magenta",
		"linear-gradient(135deg, rgb(0, 219, 222), rgb(252, 0, 255))",
	),
	grad(
		"grad-peach_coral",
		"Peach Coral",
		"linear-gradient(135deg, rgb(255, 154, 158), rgb(250, 208, 196))",
	),
	grad(
		"grad-peach_purple",
		"Peach Purple",
		"linear-gradient(135deg, rgb(246, 211, 101), rgb(253, 160, 133))",
	),
	grad(
		"grad-pink_orange_light",
		"Pink Orange Light",
		"linear-gradient(135deg, rgb(252, 203, 144), rgb(213, 126, 235))",
	),
	grad(
		"grad-yellow_cyan",
		"Yellow Cyan",
		"linear-gradient(135deg, rgb(255, 95, 109), rgb(255, 195, 113))",
	),
	grad(
		"grad-pink_yellow",
		"Pink Yellow",
		"linear-gradient(135deg, rgb(253, 187, 45), rgb(34, 193, 195))",
	),
	grad(
		"grad-pink_light",
		"Pink Light",
		"linear-gradient(135deg, rgb(212, 20, 90), rgb(251, 176, 59))",
	),
	grad(
		"grad-peach_pink",
		"Peach Pink",
		"linear-gradient(135deg, rgb(254, 225, 64), rgb(250, 112, 154))",
	),
	grad(
		"grad-yellow_blue",
		"Yellow Blue",
		"linear-gradient(135deg, rgb(255, 117, 140), rgb(255, 126, 179))",
	),
	grad(
		"grad-pink_light_2",
		"Pink Light 2",
		"linear-gradient(135deg, rgb(240, 147, 251), rgb(245, 87, 108))",
	),
	grad(
		"grad-yellow_pink",
		"Yellow Pink",
		"linear-gradient(135deg, rgb(255, 236, 210), rgb(252, 182, 159))",
	),
	grad(
		"grad-pink_light_3",
		"Pink Light 3",
		"linear-gradient(135deg, rgb(161, 140, 209), rgb(251, 194, 235))",
	),
	grad(
		"grad-peach_light",
		"Peach Light",
		"linear-gradient(135deg, rgb(169, 201, 255), rgb(255, 187, 236))",
	),
	grad(
		"grad-yellow_blue_2",
		"Yellow Blue 2",
		"linear-gradient(135deg, rgb(101, 253, 240), rgb(29, 111, 163))",
	),
	grad(
		"grad-rainbow",
		"Rainbow",
		"linear-gradient(135deg, rgb(255, 154, 139), rgb(255, 106, 136), rgb(255, 153, 172))",
	),
	grad(
		"grad-gray_red",
		"Gray Red",
		"linear-gradient(135deg, rgb(251, 218, 97), rgb(255, 90, 205))",
	),
	grad(
		"grad-navy_purple",
		"Navy Purple",
		"linear-gradient(135deg, rgb(255, 184, 184), rgb(255, 184, 209), rgb(255, 184, 233))",
	),
	grad(
		"grad-purple_orange",
		"Purple Orange",
		"linear-gradient(135deg, rgb(250, 215, 161), rgb(233, 109, 113))",
	),
	grad(
		"grad-purple_green",
		"Purple Green",
		"linear-gradient(135deg, rgb(255, 210, 111), rgb(54, 119, 255))",
	),
	grad(
		"grad-blue_cyan",
		"Blue Cyan",
		"linear-gradient(135deg, rgb(5, 25, 55), rgb(0, 77, 122), rgb(0, 135, 147), rgb(0, 191, 114), rgb(168, 235, 18))",
	),
	grad(
		"grad-purple_yellow",
		"Purple Yellow",
		"linear-gradient(135deg, rgb(51, 51, 51), rgb(221, 24, 24))",
	),
	grad(
		"grad-navy_cyan",
		"Navy Cyan",
		"linear-gradient(135deg, rgb(15, 12, 41), rgb(48, 43, 99), rgb(36, 36, 62))",
	),
	grad(
		"grad-slate_blue",
		"Slate Blue",
		"linear-gradient(135deg, rgb(35, 7, 77), rgb(204, 83, 51))",
	),
	grad(
		"grad-slate_lavender",
		"Slate Lavender",
		"linear-gradient(135deg, rgb(93, 65, 87), rgb(168, 202, 186))",
	),
	grad(
		"grad-teal_green",
		"Teal Green",
		"linear-gradient(135deg, rgb(26, 41, 128), rgb(38, 208, 206))",
	),
	grad(
		"grad-blue_orange",
		"Blue Orange",
		"linear-gradient(135deg, rgb(75, 18, 72), rgb(240, 194, 123))",
	),
	grad(
		"grad-pink_blue_2",
		"Pink Blue 2",
		"linear-gradient(135deg, rgb(0, 0, 70), rgb(28, 181, 224))",
	),
	grad(
		"grad-purple_pink",
		"Purple Pink",
		"linear-gradient(135deg, rgb(22, 34, 42), rgb(58, 96, 115))",
	),
	grad(
		"grad-purple_yellow_2",
		"Purple Yellow 2",
		"linear-gradient(135deg, rgb(31, 28, 44), rgb(146, 141, 171))",
	),
	grad(
		"grad-navy_cyan_2",
		"Navy Cyan 2",
		"linear-gradient(135deg, rgb(17, 153, 142), rgb(56, 239, 125))",
	),
	grad(
		"grad-purple_blue",
		"Purple Blue",
		"linear-gradient(135deg, rgb(16, 141, 199), rgb(239, 142, 56))",
	),
	grad(
		"grad-blue_yellow",
		"Blue Yellow",
		"linear-gradient(135deg, rgb(252, 92, 125), rgb(106, 130, 251))",
	),
	grad(
		"grad-green_lime",
		"Green Lime",
		"linear-gradient(135deg, rgb(131, 77, 155), rgb(208, 78, 214))",
	),
	grad(
		"grad-blue_green",
		"Blue Green",
		"linear-gradient(135deg, rgb(77, 160, 176), rgb(211, 157, 56))",
	),
	grad(
		"grad-red_yellow",
		"Red Yellow",
		"linear-gradient(135deg, rgb(86, 20, 176), rgb(219, 214, 92))",
	),
	grad(
		"grad-cyan_white",
		"Cyan White",
		"linear-gradient(135deg, rgb(29, 151, 108), rgb(147, 249, 185))",
	),
	grad(
		"grad-yellow_cyan_2",
		"Yellow Cyan 2",
		"linear-gradient(135deg, rgb(33, 147, 176), rgb(109, 213, 237))",
	),
	grad(
		"grad-lime_yellow",
		"Lime Yellow",
		"linear-gradient(135deg, rgb(204, 43, 94), rgb(117, 58, 136))",
	),
	grad(
		"grad-purple_peach",
		"Purple Peach",
		"linear-gradient(135deg, rgb(0, 70, 127), rgb(165, 204, 130))",
	),
	grad(
		"grad-purple_cream",
		"Purple Cream",
		"linear-gradient(135deg, rgb(248, 54, 0), rgb(249, 212, 35))",
	),
	grad(
		"grad-peach_pink_2",
		"Peach Pink 2",
		"linear-gradient(135deg, rgb(0, 255, 161), rgb(0, 255, 255))",
	),
	grad(
		"grad-mint_pink_2",
		"Mint Pink 2",
		"linear-gradient(135deg, rgb(240, 255, 0), rgb(88, 207, 251))",
	),
	grad(
		"grad-beige_purple",
		"Beige Purple",
		"linear-gradient(135deg, rgb(255, 249, 91), rgb(255, 147, 15))",
	),
	grad(
		"grad-yellow_green",
		"Yellow Green",
		"linear-gradient(135deg, rgb(252, 82, 150), rgb(246, 112, 98))",
	),
	grad("grad-pink_dark", "Pink Dark", "linear-gradient(135deg, rgb(123, 255, 0), rgb(60, 213, 0))"),
	grad(
		"grad-blue_yellow_2",
		"Blue Yellow 2",
		"linear-gradient(135deg, rgb(255, 0, 204), rgb(51, 51, 153))",
	),
	grad(
		"grad-purple_magenta",
		"Purple Magenta",
		"linear-gradient(135deg, rgb(255, 19, 97), rgb(255, 248, 0))",
	),
	grad(
		"grad-orange_red",
		"Orange Red",
		"linear-gradient(135deg, rgb(64, 224, 208), rgb(255, 140, 0), rgb(255, 0, 128))",
	),
	grad(
		"grad-blue_cyan_lime",
		"Blue Cyan Lime",
		"linear-gradient(135deg, rgb(138, 35, 135), rgb(233, 64, 87), rgb(242, 113, 33))",
	),
	grad(
		"grad-blue_purple_2",
		"Blue Purple 2",
		"linear-gradient(135deg, rgb(255, 239, 186), rgb(255, 255, 255))",
	),
	grad(
		"grad-red_orange",
		"Red Orange",
		"linear-gradient(135deg, rgb(161, 255, 206), rgb(250, 255, 209))",
	),
	grad(
		"grad-gradient_88",
		"Gradient 88",
		"linear-gradient(135deg, rgb(243, 249, 167), rgb(202, 197, 49))",
	),
	grad(
		"grad-gradient_89",
		"Gradient 89",
		"linear-gradient(135deg, rgb(221, 214, 243), rgb(250, 172, 168))",
	),
	grad(
		"grad-gradient_90",
		"Gradient 90",
		"linear-gradient(135deg, rgb(232, 219, 252), rgb(248, 249, 210))",
	),
	grad(
		"grad-gradient_91",
		"Gradient 91",
		"linear-gradient(135deg, rgb(238, 205, 163), rgb(239, 98, 159))",
	),
	grad(
		"grad-gradient_92",
		"Gradient 92",
		"linear-gradient(135deg, rgb(201, 255, 191), rgb(255, 175, 189))",
	),
	grad(
		"grad-gradient_93",
		"Gradient 93",
		"linear-gradient(135deg, rgb(232, 203, 192), rgb(99, 111, 164))",
	),
	grad(
		"grad-gradient_94",
		"Gradient 94",
		"linear-gradient(135deg, rgb(220, 227, 91), rgb(69, 182, 73))",
	),
	grad(
		"grad-gradient_95",
		"Gradient 95",
		"linear-gradient(135deg, rgb(255, 0, 153), rgb(73, 50, 64))",
	),
	grad(
		"grad-gradient_96",
		"Gradient 96",
		"linear-gradient(135deg, rgb(0, 79, 249), rgb(255, 249, 76))",
	),
	grad(
		"grad-gradient_97",
		"Gradient 97",
		"linear-gradient(135deg, rgb(127, 0, 255), rgb(225, 0, 255))",
	),
	grad(
		"grad-gradient_98",
		"Gradient 98",
		"linear-gradient(135deg, rgb(253, 200, 48), rgb(243, 115, 53))",
	),
	grad(
		"grad-gradient_99",
		"Gradient 99",
		"linear-gradient(135deg, rgb(237, 33, 58), rgb(147, 41, 30))",
	),
	grad(
		"grad-gradient_100",
		"Gradient 100",
		"linear-gradient(135deg, rgb(31, 162, 255), rgb(18, 216, 250), rgb(166, 255, 203))",
	),
	grad(
		"grad-gradient_101",
		"Gradient 101",
		"linear-gradient(135deg, rgb(69, 104, 220), rgb(176, 106, 179))",
	),
	grad(
		"grad-gradient_102",
		"Gradient 102",
		"linear-gradient(135deg, rgb(255, 81, 47), rgb(221, 36, 118))",
	),
];

/** 100 dark "magic" gradients (radial/conic/pattern glows). */
export const REF_MAGIC: BackgroundPreset[] = [
	grad(
		"magic-gold_center",
		"Gold Center",
		"radial-gradient(circle, rgb(172, 116, 42) 0%, transparent 50%, black 100%)",
	),
	grad(
		"magic-amber_center",
		"Amber Center",
		"radial-gradient(circle, rgb(255, 184, 44) 0%, transparent 50%, black 100%)",
	),
	grad(
		"magic-silver_center",
		"Silver Center",
		"radial-gradient(circle, rgb(220, 220, 223) 0%, transparent 50%, black 100%)",
	),
	grad(
		"magic-cyan_center",
		"Cyan Center",
		"radial-gradient(circle, rgb(98, 185, 220) 0%, transparent 50%, black 100%)",
	),
	grad(
		"magic-olive_center",
		"Olive Center",
		"radial-gradient(circle, rgb(188, 189, 125) 0%, transparent 50%, black 100%)",
	),
	grad(
		"magic-teal_center",
		"Teal Center",
		"radial-gradient(circle, rgb(0, 132, 91) 0%, transparent 50%, black 100%)",
	),
	grad(
		"magic-mint_center",
		"Mint Center",
		"radial-gradient(circle, rgb(0, 233, 161) 0%, transparent 50%, black 100%)",
	),
	grad(
		"magic-gold_ring",
		"Gold Ring",
		"radial-gradient(circle, transparent 0%, rgb(172, 116, 42) 50%, transparent 70%, black 100%)",
	),
	grad(
		"magic-mint_ring",
		"Mint Ring",
		"radial-gradient(circle, transparent 0%, rgb(0, 233, 161) 50%, transparent 70%, black 100%)",
	),
	grad(
		"magic-orange_ring",
		"Orange Ring",
		"radial-gradient(circle, transparent 0%, rgb(255, 77, 0) 50%, transparent 70%, black 100%)",
	),
	grad("magic-orange_glow", "Orange Glow", "radial-gradient(rgb(255, 77, 0) 0%, black 80%)"),
	grad("magic-cyan_glow", "Cyan Glow", "radial-gradient(rgb(98, 185, 220) 0%, black 80%)"),
	grad(
		"magic-silver_topleft",
		"Silver Topleft",
		"radial-gradient(circle at left top, rgb(220, 220, 223) 0%, black 100%)",
	),
	grad(
		"magic-gold_topleft",
		"Gold Topleft",
		"radial-gradient(circle at left top, rgb(172, 116, 42) 0%, black 100%)",
	),
	grad(
		"magic-cyan_topright",
		"Cyan Topright",
		"radial-gradient(circle at 70% 30%, rgb(98, 185, 220) 0%, black 100%)",
	),
	grad(
		"magic-silver_topright",
		"Silver Topright",
		"radial-gradient(circle at 70% 30%, rgb(220, 220, 223) 0%, black 100%)",
	),
	grad(
		"magic-olive_topright",
		"Olive Topright",
		"radial-gradient(circle at 70% 30%, rgb(188, 189, 125) 0%, black 100%)",
	),
	grad(
		"magic-amber_topright",
		"Amber Topright",
		"radial-gradient(circle at 70% 30%, rgb(255, 184, 44) 0%, black 100%)",
	),
	grad(
		"magic-gray_topright",
		"Gray Topright",
		"radial-gradient(circle at 70% 30%, rgb(119, 119, 136) 0%, black 100%)",
	),
	grad(
		"magic-silver_bottomleft",
		"Silver Bottomleft",
		"radial-gradient(circle at 30% 70%, rgb(220, 220, 223) 0%, black 100%)",
	),
	grad(
		"magic-gold_bottomleft",
		"Gold Bottomleft",
		"radial-gradient(circle at 30% 70%, rgb(172, 116, 42) 0%, black 100%)",
	),
	grad(
		"magic-cyan_bottomleft",
		"Cyan Bottomleft",
		"radial-gradient(circle at 30% 70%, rgb(98, 185, 220) 0%, black 100%)",
	),
	grad(
		"magic-amber_bottom",
		"Amber Bottom",
		"radial-gradient(circle at center bottom, rgb(255, 184, 44) 0%, black 100%)",
	),
	grad(
		"magic-olive_bottom",
		"Olive Bottom",
		"radial-gradient(circle at center bottom, rgb(188, 189, 125) 0%, black 100%)",
	),
	grad(
		"magic-teal_left",
		"Teal Left",
		"radial-gradient(circle at 30% 50%, rgb(0, 132, 91) 0%, black 75%)",
	),
	grad(
		"magic-mint_left",
		"Mint Left",
		"radial-gradient(circle at 30% 50%, rgb(0, 233, 161) 0%, black 75%)",
	),
	grad(
		"magic-amber_left",
		"Amber Left",
		"radial-gradient(circle at 30% 50%, rgb(255, 184, 44) 0%, black 75%)",
	),
	grad(
		"magic-orange_left",
		"Orange Left",
		"radial-gradient(circle at 30% 50%, rgb(255, 77, 0) 0%, black 75%)",
	),
	grad(
		"magic-dark_left",
		"Dark Left",
		"radial-gradient(circle at 30% 50%, rgb(42, 42, 46) 0%, black 75%)",
	),
	grad(
		"magic-amber_below",
		"Amber Below",
		"radial-gradient(circle at 50% 120%, rgb(255, 184, 44) 0%, black 75%)",
	),
	grad(
		"magic-dark_below",
		"Dark Below",
		"radial-gradient(circle at 50% 120%, rgb(42, 42, 46) 0%, black 75%)",
	),
	grad(
		"magic-gold_diagonal",
		"Gold Diagonal",
		"radial-gradient(circle at 25% 25%, rgb(172, 116, 42) 0%, transparent 50%), radial-gradient(circle at 75% 75%, rgb(172, 116, 42) 0%, transparent 50%), linear-gradient(135deg, rgb(7, 7, 7) 0%, rgb(12, 12, 12) 100%)",
	),
	grad(
		"magic-amber_diagonal",
		"Amber Diagonal",
		"radial-gradient(circle at 25% 25%, rgb(255, 184, 44) 0%, transparent 50%), radial-gradient(circle at 75% 75%, rgb(255, 184, 44) 0%, transparent 50%), linear-gradient(135deg, rgb(7, 7, 7) 0%, rgb(12, 12, 12) 100%)",
	),
	grad(
		"magic-silver_diagonal",
		"Silver Diagonal",
		"radial-gradient(circle at 25% 25%, rgb(220, 220, 223) 0%, transparent 50%), radial-gradient(circle at 75% 75%, rgb(220, 220, 223) 0%, transparent 50%), linear-gradient(135deg, rgb(7, 7, 7) 0%, rgb(12, 12, 12) 100%)",
	),
	grad(
		"magic-cyan_diagonal",
		"Cyan Diagonal",
		"radial-gradient(circle at 25% 25%, rgb(98, 185, 220) 0%, transparent 50%), radial-gradient(circle at 75% 75%, rgb(98, 185, 220) 0%, transparent 50%), linear-gradient(135deg, rgb(7, 7, 7) 0%, rgb(12, 12, 12) 100%)",
	),
	grad(
		"magic-orange_vertical",
		"Orange Vertical",
		"radial-gradient(at center top, rgb(255, 77, 0) 0%, transparent 70%), radial-gradient(at center bottom, rgb(255, 77, 0) 0%, transparent 70%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gold_vertical",
		"Gold Vertical",
		"radial-gradient(at center top, rgb(172, 116, 42) 0%, transparent 70%), radial-gradient(at center bottom, rgb(172, 116, 42) 0%, transparent 70%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-mint_vertical",
		"Mint Vertical",
		"radial-gradient(at center top, rgb(0, 233, 161) 0%, transparent 70%), radial-gradient(at center bottom, rgb(0, 233, 161) 0%, transparent 70%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gray_vertical",
		"Gray Vertical",
		"radial-gradient(at center top, rgb(119, 119, 136) 0%, transparent 70%), radial-gradient(at center bottom, rgb(119, 119, 136) 0%, transparent 70%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-olive_vertical",
		"Olive Vertical",
		"radial-gradient(at center top, rgb(188, 189, 125) 0%, transparent 70%), radial-gradient(at center bottom, rgb(188, 189, 125) 0%, transparent 70%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-cyan_vertical",
		"Cyan Vertical",
		"radial-gradient(at center top, rgb(98, 185, 220) 0%, transparent 70%), radial-gradient(at center bottom, rgb(98, 185, 220) 0%, transparent 70%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-olive_gold_vertical",
		"Olive Gold Vertical",
		"radial-gradient(at center bottom, rgb(188, 189, 125) 0%, transparent 60%), radial-gradient(at center top, rgb(172, 116, 42) 0%, transparent 60%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-amber_olive_vertical",
		"Amber Olive Vertical",
		"radial-gradient(at center bottom, rgb(255, 184, 44) 0%, transparent 60%), radial-gradient(at center top, rgb(188, 189, 125) 0%, transparent 60%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-orange_mint_vertical",
		"Orange Mint Vertical",
		"radial-gradient(at center bottom, rgb(255, 77, 0) 0%, transparent 60%), radial-gradient(at center top, rgb(0, 233, 161) 0%, transparent 60%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-amber_dark_corners",
		"Amber Dark Corners",
		"radial-gradient(circle at 20% 20%, rgb(42, 42, 46) 0%, transparent 40%), radial-gradient(circle at 80% 20%, rgb(255, 184, 44) 0%, transparent 40%), radial-gradient(circle at 20% 80%, rgb(255, 184, 44) 0%, transparent 40%), radial-gradient(circle at 80% 80%, rgb(42, 42, 46) 0%, transparent 40%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-amber_teal",
		"Amber Teal",
		"radial-gradient(circle at 30% 70%, rgb(255, 184, 44) 0%, transparent 50%), radial-gradient(circle at 70% 30%, rgb(0, 132, 91) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gray_gold_teal",
		"Gray Gold Teal",
		"radial-gradient(circle at 20% 80%, rgb(119, 119, 136) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(172, 116, 42) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(0, 132, 91) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-orange_gray_olive",
		"Orange Gray Olive",
		"radial-gradient(circle at 20% 80%, rgb(255, 77, 0) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(119, 119, 136) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(188, 189, 125) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-orange_cyan_gold",
		"Orange Cyan Gold",
		"radial-gradient(circle at 20% 80%, rgb(255, 77, 0) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(98, 185, 220) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(172, 116, 42) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_gray_amber",
		"Dark Gray Amber",
		"radial-gradient(circle at 20% 80%, rgb(42, 42, 46) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(119, 119, 136) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(255, 184, 44) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_amber_silver",
		"Dark Amber Silver",
		"radial-gradient(circle at 20% 80%, rgb(42, 42, 46) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(255, 184, 44) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(220, 220, 223) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_amber_mint",
		"Dark Amber Mint",
		"radial-gradient(circle at 20% 80%, rgb(42, 42, 46) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(255, 184, 44) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(0, 233, 161) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_orange_gray",
		"Dark Orange Gray",
		"radial-gradient(circle at 20% 80%, rgb(42, 42, 46) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(255, 77, 0) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(119, 119, 136) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_orange_mint",
		"Dark Orange Mint",
		"radial-gradient(circle at 20% 80%, rgb(42, 42, 46) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(255, 77, 0) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(0, 233, 161) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_orange_silver",
		"Dark Orange Silver",
		"radial-gradient(circle at 20% 80%, rgb(42, 42, 46) 0%, transparent 50%), radial-gradient(circle at 50% 30%, rgb(255, 77, 0) 0%, transparent 50%), radial-gradient(circle at 80% 70%, rgb(220, 220, 223) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-mint_silver_orb",
		"Mint Silver Orb",
		"radial-gradient(circle at 25% 25%, rgb(42, 42, 46) 0%, transparent 35%), radial-gradient(circle, rgb(0, 233, 161) 0%, transparent 45%), radial-gradient(circle at 75% 75%, rgb(220, 220, 223) 0%, transparent 35%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gray_gold_orb",
		"Gray Gold Orb",
		"radial-gradient(circle at 25% 25%, rgb(42, 42, 46) 0%, transparent 35%), radial-gradient(circle, rgb(119, 119, 136) 0%, transparent 45%), radial-gradient(circle at 75% 75%, rgb(172, 116, 42) 0%, transparent 35%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-olive_gold_orb",
		"Olive Gold Orb",
		"radial-gradient(circle at 25% 25%, rgb(42, 42, 46) 0%, transparent 35%), radial-gradient(circle, rgb(188, 189, 125) 0%, transparent 45%), radial-gradient(circle at 75% 75%, rgb(172, 116, 42) 0%, transparent 35%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-orange_teal_orb",
		"Orange Teal Orb",
		"radial-gradient(circle at 25% 25%, rgb(42, 42, 46) 0%, transparent 35%), radial-gradient(circle, rgb(255, 77, 0) 0%, transparent 45%), radial-gradient(circle at 75% 75%, rgb(0, 132, 91) 0%, transparent 35%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-orange_silver_orb",
		"Orange Silver Orb",
		"radial-gradient(circle at 25% 25%, rgb(42, 42, 46) 0%, transparent 35%), radial-gradient(circle, rgb(255, 77, 0) 0%, transparent 45%), radial-gradient(circle at 75% 75%, rgb(220, 220, 223) 0%, transparent 35%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-orange_amber_orb",
		"Orange Amber Orb",
		"radial-gradient(circle at 25% 25%, rgb(42, 42, 46) 0%, transparent 35%), radial-gradient(circle, rgb(255, 77, 0) 0%, transparent 45%), radial-gradient(circle at 75% 75%, rgb(255, 184, 44) 0%, transparent 35%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gray_nebula",
		"Gray Nebula",
		"radial-gradient(circle at 20% 20%, rgba(169, 169, 186, 0.7) 0%, transparent 30%), radial-gradient(circle at 40% 60%, rgba(69, 69, 86, 0.8) 0%, transparent 40%), radial-gradient(circle at 60% 30%, rgba(169, 169, 186, 0.7) 0%, transparent 35%), radial-gradient(circle at 80% 70%, rgba(69, 69, 86, 0.8) 0%, transparent 25%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gold_nebula",
		"Gold Nebula",
		"radial-gradient(circle at 20% 20%, rgba(222, 166, 92, 0.7) 0%, transparent 30%), radial-gradient(circle at 40% 60%, rgba(122, 66, 0, 0.8) 0%, transparent 40%), radial-gradient(circle at 60% 30%, rgba(222, 166, 92, 0.7) 0%, transparent 35%), radial-gradient(circle at 80% 70%, rgba(122, 66, 0, 0.8) 0%, transparent 25%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-olive_nebula",
		"Olive Nebula",
		"radial-gradient(circle at 20% 20%, rgba(238, 239, 175, 0.7) 0%, transparent 30%), radial-gradient(circle at 40% 60%, rgba(138, 139, 75, 0.8) 0%, transparent 40%), radial-gradient(circle at 60% 30%, rgba(238, 239, 175, 0.7) 0%, transparent 35%), radial-gradient(circle at 80% 70%, rgba(138, 139, 75, 0.8) 0%, transparent 25%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-mint_nebula",
		"Mint Nebula",
		"radial-gradient(circle at 20% 20%, rgba(50, 255, 211, 0.7) 0%, transparent 30%), radial-gradient(circle at 40% 60%, rgba(0, 183, 111, 0.8) 0%, transparent 40%), radial-gradient(circle at 60% 30%, rgba(50, 255, 211, 0.7) 0%, transparent 35%), radial-gradient(circle at 80% 70%, rgba(0, 183, 111, 0.8) 0%, transparent 25%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-amber_olive_scatter",
		"Amber Olive Scatter",
		"radial-gradient(circle at 10% 20%, rgb(255, 184, 44) 0%, transparent 30%), radial-gradient(circle at 80% 30%, rgb(188, 189, 125) 0%, transparent 30%), radial-gradient(circle at 40% 70%, rgb(255, 184, 44) 0%, transparent 30%), radial-gradient(at 60% 80%, rgb(188, 189, 125) 0%, transparent 40%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gray_constellation",
		"Gray Constellation",
		"radial-gradient(circle at 10% 10%, rgb(119, 119, 136) 0%, transparent 15%), radial-gradient(circle at 30% 20%, rgb(119, 119, 136) 0%, transparent 10%), radial-gradient(circle, rgb(119, 119, 136) 0%, transparent 25%), radial-gradient(circle at 70% 30%, rgb(119, 119, 136) 0%, transparent 15%), radial-gradient(circle at 90% 60%, rgb(119, 119, 136) 0%, transparent 20%), radial-gradient(circle at 20% 80%, rgb(119, 119, 136) 0%, transparent 15%), radial-gradient(circle at 40% 70%, rgb(119, 119, 136) 0%, transparent 10%), radial-gradient(circle at 60% 90%, rgb(119, 119, 136) 0%, transparent 15%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_constellation",
		"Dark Constellation",
		"radial-gradient(circle at 10% 10%, rgb(42, 42, 46) 0%, transparent 15%), radial-gradient(circle at 30% 20%, rgb(42, 42, 46) 0%, transparent 10%), radial-gradient(circle, rgb(42, 42, 46) 0%, transparent 25%), radial-gradient(circle at 70% 30%, rgb(42, 42, 46) 0%, transparent 15%), radial-gradient(circle at 90% 60%, rgb(42, 42, 46) 0%, transparent 20%), radial-gradient(circle at 20% 80%, rgb(42, 42, 46) 0%, transparent 15%), radial-gradient(circle at 40% 70%, rgb(42, 42, 46) 0%, transparent 10%), radial-gradient(circle at 60% 90%, rgb(42, 42, 46) 0%, transparent 15%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-cyan_constellation",
		"Cyan Constellation",
		"radial-gradient(circle at 10% 10%, rgb(98, 185, 220) 0%, transparent 15%), radial-gradient(circle at 30% 20%, rgb(98, 185, 220) 0%, transparent 10%), radial-gradient(circle, rgb(98, 185, 220) 0%, transparent 25%), radial-gradient(circle at 70% 30%, rgb(98, 185, 220) 0%, transparent 15%), radial-gradient(circle at 90% 60%, rgb(98, 185, 220) 0%, transparent 20%), radial-gradient(circle at 20% 80%, rgb(98, 185, 220) 0%, transparent 15%), radial-gradient(circle at 40% 70%, rgb(98, 185, 220) 0%, transparent 10%), radial-gradient(circle at 60% 90%, rgb(98, 185, 220) 0%, transparent 15%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-cyan_rotate",
		"Cyan Rotate",
		"conic-gradient(rgb(98, 185, 220) 0deg, rgb(98, 185, 220) 45deg, black 45deg, black 90deg, rgb(98, 185, 220) 90deg, rgb(98, 185, 220) 135deg, black 135deg, black 180deg, rgb(98, 185, 220) 180deg, rgb(98, 185, 220) 225deg, black 225deg, black 270deg, rgb(98, 185, 220) 270deg, rgb(98, 185, 220) 315deg, black 315deg, black 360deg)",
	),
	grad(
		"magic-amber_corners",
		"Amber Corners",
		"conic-gradient(at 25% 25%, rgb(255, 184, 44) 0deg, black 90deg, transparent 180deg), conic-gradient(from 90deg at 75% 25%, rgb(255, 184, 44) 0deg, black 90deg, transparent 180deg), conic-gradient(from 180deg at 75% 75%, rgb(255, 184, 44) 0deg, black 90deg, transparent 180deg), conic-gradient(from 270deg at 25% 75%, rgb(255, 184, 44) 0deg, black 90deg, transparent 180deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-olive_corners",
		"Olive Corners",
		"conic-gradient(at 25% 25%, rgb(188, 189, 125) 0deg, black 90deg, transparent 180deg), conic-gradient(from 90deg at 75% 25%, rgb(188, 189, 125) 0deg, black 90deg, transparent 180deg), conic-gradient(from 180deg at 75% 75%, rgb(188, 189, 125) 0deg, black 90deg, transparent 180deg), conic-gradient(from 270deg at 25% 75%, rgb(188, 189, 125) 0deg, black 90deg, transparent 180deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_corners",
		"Dark Corners",
		"conic-gradient(at 25% 25%, rgb(42, 42, 46) 0deg, black 90deg, transparent 180deg), conic-gradient(from 90deg at 75% 25%, rgb(42, 42, 46) 0deg, black 90deg, transparent 180deg), conic-gradient(from 180deg at 75% 75%, rgb(42, 42, 46) 0deg, black 90deg, transparent 180deg), conic-gradient(from 270deg at 25% 75%, rgb(42, 42, 46) 0deg, black 90deg, transparent 180deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-mint_gold_teal",
		"Mint Gold Teal",
		"conic-gradient(rgb(0, 233, 161) 0deg, transparent 60deg, rgb(172, 116, 42) 120deg, transparent 180deg, rgb(0, 132, 91) 240deg, transparent 300deg, rgb(0, 233, 161) 360deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-silver_olive_gold",
		"Silver Olive Gold",
		"conic-gradient(rgb(220, 220, 223) 0deg, transparent 60deg, rgb(188, 189, 125) 120deg, transparent 180deg, rgb(172, 116, 42) 240deg, transparent 300deg, rgb(220, 220, 223) 360deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-silver_teal_dark",
		"Silver Teal Dark",
		"conic-gradient(rgb(220, 220, 223) 0deg, transparent 60deg, rgb(0, 132, 91) 120deg, transparent 180deg, rgb(42, 42, 46) 240deg, transparent 300deg, rgb(220, 220, 223) 360deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-orange_stripes",
		"Orange Stripes",
		"conic-gradient(rgb(255, 77, 0) 0deg, rgb(255, 77, 0) 60deg, transparent 60deg, transparent 120deg, rgb(255, 77, 0) 120deg, rgb(255, 77, 0) 180deg, transparent 180deg, transparent 240deg, rgb(255, 77, 0) 240deg, rgb(255, 77, 0) 300deg, transparent 300deg, transparent 360deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gray_stripes",
		"Gray Stripes",
		"conic-gradient(rgb(119, 119, 136) 0deg, rgb(119, 119, 136) 60deg, transparent 60deg, transparent 120deg, rgb(119, 119, 136) 120deg, rgb(119, 119, 136) 180deg, transparent 180deg, transparent 240deg, rgb(119, 119, 136) 240deg, rgb(119, 119, 136) 300deg, transparent 300deg, transparent 360deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-silver_stripes",
		"Silver Stripes",
		"conic-gradient(rgb(220, 220, 223) 0deg, rgb(220, 220, 223) 60deg, transparent 60deg, transparent 120deg, rgb(220, 220, 223) 120deg, rgb(220, 220, 223) 180deg, transparent 180deg, transparent 240deg, rgb(220, 220, 223) 240deg, rgb(220, 220, 223) 300deg, transparent 300deg, transparent 360deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_stripes",
		"Dark Stripes",
		"conic-gradient(rgb(42, 42, 46) 0deg, rgb(42, 42, 46) 60deg, transparent 60deg, transparent 120deg, rgb(42, 42, 46) 120deg, rgb(42, 42, 46) 180deg, transparent 180deg, transparent 240deg, rgb(42, 42, 46) 240deg, rgb(42, 42, 46) 300deg, transparent 300deg, transparent 360deg), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-amber_mint_x",
		"Amber Mint X",
		"linear-gradient(45deg, transparent 40%, rgb(255, 184, 44) 40%, rgb(255, 184, 44) 60%, transparent 60%), linear-gradient(135deg, transparent 40%, rgb(0, 233, 161) 40%, rgb(0, 233, 161) 60%, transparent 60%), radial-gradient(circle, rgb(220, 220, 223) 0%, transparent 50%), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-mint_grid",
		"Mint Grid",
		"repeating-linear-gradient(0deg, transparent, transparent 10px, rgb(0, 233, 161) 10px, rgb(0, 233, 161) 11px), repeating-linear-gradient(90deg, transparent, transparent 10px, rgb(0, 233, 161) 10px, rgb(0, 233, 161) 11px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gold_grid",
		"Gold Grid",
		"repeating-linear-gradient(0deg, transparent, transparent 10px, rgb(172, 116, 42) 10px, rgb(172, 116, 42) 11px), repeating-linear-gradient(90deg, transparent, transparent 10px, rgb(172, 116, 42) 10px, rgb(172, 116, 42) 11px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-silver_grid",
		"Silver Grid",
		"repeating-linear-gradient(0deg, transparent, transparent 10px, rgb(220, 220, 223) 10px, rgb(220, 220, 223) 11px), repeating-linear-gradient(90deg, transparent, transparent 10px, rgb(220, 220, 223) 10px, rgb(220, 220, 223) 11px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-teal_diagonal",
		"Teal Diagonal",
		"repeating-linear-gradient(45deg, black, black 5px, rgb(0, 132, 91) 5px, rgb(0, 132, 91) 10px, black 10px, black 15px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_diagonal",
		"Dark Diagonal",
		"repeating-linear-gradient(45deg, black, black 5px, rgb(42, 42, 46) 5px, rgb(42, 42, 46) 10px, black 10px, black 15px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-silver_crosshatch",
		"Silver Crosshatch",
		"repeating-linear-gradient(45deg, transparent, transparent 5px, rgba(220, 220, 223, 0.2) 5px, rgba(220, 220, 223, 0.2) 10px), repeating-linear-gradient(135deg, transparent, transparent 5px, rgba(220, 220, 223, 0.2) 5px, rgba(220, 220, 223, 0.2) 10px), linear-gradient(rgb(15, 15, 15) 0%, rgb(15, 15, 15) 100%)",
	),
	grad(
		"magic-amber_crosshatch",
		"Amber Crosshatch",
		"repeating-linear-gradient(45deg, transparent, transparent 5px, rgba(255, 184, 44, 0.2) 5px, rgba(255, 184, 44, 0.2) 10px), repeating-linear-gradient(135deg, transparent, transparent 5px, rgba(255, 184, 44, 0.2) 5px, rgba(255, 184, 44, 0.2) 10px), linear-gradient(rgb(15, 15, 15) 0%, rgb(15, 15, 15) 100%)",
	),
	grad(
		"magic-gray_crosshatch",
		"Gray Crosshatch",
		"repeating-linear-gradient(45deg, rgba(119, 119, 136, 0.05), rgba(119, 119, 136, 0.05) 1px, transparent 1px, transparent 5px), repeating-linear-gradient(135deg, rgba(119, 119, 136, 0.05), rgba(119, 119, 136, 0.05) 1px, transparent 1px, transparent 5px), linear-gradient(rgba(10, 10, 10, 0.9) 0%, rgba(10, 10, 10, 0.9) 100%)",
	),
	grad(
		"magic-mint_noise",
		"Mint Noise",
		"repeating-conic-gradient(rgba(0, 233, 161, 0.05) 0deg, transparent 1deg, rgba(0, 233, 161, 0.05) 2deg), linear-gradient(rgba(10, 10, 10, 0.9) 0%, rgba(10, 10, 10, 0.9) 100%)",
	),
	grad(
		"magic-olive_noise",
		"Olive Noise",
		"repeating-conic-gradient(rgba(188, 189, 125, 0.05) 0deg, transparent 1deg, rgba(188, 189, 125, 0.05) 2deg), linear-gradient(rgba(10, 10, 10, 0.9) 0%, rgba(10, 10, 10, 0.9) 100%)",
	),
	grad(
		"magic-orange_noise",
		"Orange Noise",
		"repeating-conic-gradient(rgba(255, 77, 0, 0.05) 0deg, transparent 1deg, rgba(255, 77, 0, 0.05) 2deg), linear-gradient(rgba(10, 10, 10, 0.9) 0%, rgba(10, 10, 10, 0.9) 100%)",
	),
	grad(
		"magic-cyan_dots",
		"Cyan Dots",
		"repeating-radial-gradient(circle at 25% 25%, transparent 0px, rgb(98, 185, 220) 1px, transparent 2px), repeating-radial-gradient(circle at 75% 75%, transparent 0px, rgb(98, 185, 220) 1px, transparent 2px), repeating-radial-gradient(circle, transparent 0px, rgb(98, 185, 220) 1px, transparent 2px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-gold_dots",
		"Gold Dots",
		"repeating-radial-gradient(circle at 25% 25%, transparent 0px, rgb(172, 116, 42) 1px, transparent 2px), repeating-radial-gradient(circle at 75% 75%, transparent 0px, rgb(172, 116, 42) 1px, transparent 2px), repeating-radial-gradient(circle, transparent 0px, rgb(172, 116, 42) 1px, transparent 2px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-teal_dots",
		"Teal Dots",
		"repeating-radial-gradient(circle at 25% 25%, transparent 0px, rgb(0, 132, 91) 1px, transparent 2px), repeating-radial-gradient(circle at 75% 75%, transparent 0px, rgb(0, 132, 91) 1px, transparent 2px), repeating-radial-gradient(circle, transparent 0px, rgb(0, 132, 91) 1px, transparent 2px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-dark_dots",
		"Dark Dots",
		"repeating-radial-gradient(circle at 25% 25%, transparent 0px, rgb(42, 42, 46) 1px, transparent 2px), repeating-radial-gradient(circle at 75% 75%, transparent 0px, rgb(42, 42, 46) 1px, transparent 2px), repeating-radial-gradient(circle, transparent 0px, rgb(42, 42, 46) 1px, transparent 2px), linear-gradient(135deg, rgb(10, 10, 10) 0%, rgb(10, 10, 10) 100%)",
	),
	grad(
		"magic-mint_soft_dots",
		"Mint Soft Dots",
		"repeating-radial-gradient(circle at 25% 25%, transparent 0px, rgba(0, 233, 161, 0.1) 1px, transparent 2px), repeating-radial-gradient(circle at 75% 75%, transparent 0px, rgba(0, 233, 161, 0.1) 1px, transparent 3px), linear-gradient(rgba(10, 10, 10, 0.9) 0%, rgba(10, 10, 10, 0.9) 100%)",
	),
	grad(
		"magic-cyan_starfield",
		"Cyan Starfield",
		"repeating-radial-gradient(circle at 10% 10%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 20% 20%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 30% 30%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 40% 40%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 60% 60%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 70% 70%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 80% 80%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 90% 90%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 15% 45%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 35% 65%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 55% 85%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 75% 25%, transparent 0px, rgba(98, 185, 220, 0.15) 1px, transparent 1px), linear-gradient(rgb(12, 12, 12) 0%, rgb(15, 15, 15) 100%)",
	),
	grad(
		"magic-gold_starfield",
		"Gold Starfield",
		"repeating-radial-gradient(circle at 10% 10%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 20% 20%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 30% 30%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 40% 40%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 60% 60%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 70% 70%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 80% 80%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 90% 90%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 15% 45%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 35% 65%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 55% 85%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 75% 25%, transparent 0px, rgba(172, 116, 42, 0.15) 1px, transparent 1px), linear-gradient(rgb(12, 12, 12) 0%, rgb(15, 15, 15) 100%)",
	),
	grad(
		"magic-silver_starfield",
		"Silver Starfield",
		"repeating-radial-gradient(circle at 10% 10%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 20% 20%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 30% 30%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 40% 40%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 60% 60%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 70% 70%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 80% 80%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 90% 90%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 15% 45%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 35% 65%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 55% 85%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), repeating-radial-gradient(circle at 75% 25%, transparent 0px, rgba(220, 220, 223, 0.15) 1px, transparent 1px), linear-gradient(rgb(12, 12, 12) 0%, rgb(15, 15, 15) 100%)",
	),
];

/** 12 soft mesh gradients. */
export const REF_MESH: BackgroundPreset[] = [
	grad(
		"mesh-aurora",
		"Aurora",
		"radial-gradient(at 40% 20%, hsla(330,100%,75%,1) 0px, transparent 50%), radial-gradient(at 80% 0%, hsla(190,100%,75%,1) 0px, transparent 50%), radial-gradient(at 0% 50%, hsla(270,100%,75%,1) 0px, transparent 50%), radial-gradient(at 80% 50%, hsla(39,100%,75%,1) 0px, transparent 50%), radial-gradient(at 0% 100%, hsla(210,100%,75%,1) 0px, transparent 50%), radial-gradient(at 80% 100%, hsla(150,100%,75%,1) 0px, transparent 50%), radial-gradient(at 0% 0%, hsla(60,100%,75%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-sunset",
		"Sunset",
		"radial-gradient(at 0% 0%, hsla(355,85%,65%,1) 0px, transparent 50%), radial-gradient(at 100% 0%, hsla(30,100%,75%,1) 0px, transparent 50%), radial-gradient(at 100% 100%, hsla(290,85%,55%,1) 0px, transparent 50%), radial-gradient(at 0% 100%, hsla(15,100%,60%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-ocean",
		"Ocean",
		"radial-gradient(at 50% 0%, hsla(200,100%,75%,1) 0px, transparent 50%), radial-gradient(at 100% 50%, hsla(190,100%,60%,1) 0px, transparent 50%), radial-gradient(at 0% 50%, hsla(180,100%,65%,1) 0px, transparent 50%), radial-gradient(at 50% 100%, hsla(220,100%,45%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-forest",
		"Forest",
		"radial-gradient(at 40% 40%, hsla(120,80%,60%,1) 0px, transparent 50%), radial-gradient(at 80% 20%, hsla(90,70%,75%,1) 0px, transparent 50%), radial-gradient(at 20% 80%, hsla(150,80%,45%,1) 0px, transparent 50%), radial-gradient(at 90% 90%, hsla(60,100%,65%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-candy",
		"Candy",
		"radial-gradient(at 30% 30%, hsla(340,100%,75%,1) 0px, transparent 50%), radial-gradient(at 70% 20%, hsla(280,100%,75%,1) 0px, transparent 50%), radial-gradient(at 20% 70%, hsla(45,100%,80%,1) 0px, transparent 50%), radial-gradient(at 80% 80%, hsla(330,100%,70%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-cosmic",
		"Cosmic",
		"radial-gradient(at 10% 10%, hsla(260,100%,40%,1) 0px, transparent 50%), radial-gradient(at 90% 10%, hsla(300,100%,50%,1) 0px, transparent 50%), radial-gradient(at 50% 50%, hsla(220,100%,30%,1) 0px, transparent 50%), radial-gradient(at 90% 90%, hsla(190,100%,60%,1) 0px, transparent 50%), radial-gradient(at 10% 90%, hsla(270,100%,60%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-peach",
		"Peach",
		"radial-gradient(at 0% 0%, hsla(25,100%,85%,1) 0px, transparent 50%), radial-gradient(at 100% 0%, hsla(340,100%,85%,1) 0px, transparent 50%), radial-gradient(at 100% 100%, hsla(30,100%,75%,1) 0px, transparent 50%), radial-gradient(at 0% 100%, hsla(15,100%,80%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-lavender",
		"Lavender",
		"radial-gradient(at 20% 20%, hsla(280,80%,80%,1) 0px, transparent 50%), radial-gradient(at 80% 20%, hsla(260,90%,85%,1) 0px, transparent 50%), radial-gradient(at 50% 80%, hsla(300,70%,75%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-mint",
		"Mint",
		"radial-gradient(at 40% 20%, hsla(160,80%,75%,1) 0px, transparent 50%), radial-gradient(at 80% 60%, hsla(180,70%,80%,1) 0px, transparent 50%), radial-gradient(at 0% 80%, hsla(140,80%,70%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-rose",
		"Rose",
		"radial-gradient(at 30% 30%, hsla(350,90%,80%,1) 0px, transparent 50%), radial-gradient(at 70% 70%, hsla(330,80%,75%,1) 0px, transparent 50%), radial-gradient(at 70% 30%, hsla(10,90%,85%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-electric",
		"Electric",
		"radial-gradient(at 0% 50%, hsla(180,100%,50%,1) 0px, transparent 50%), radial-gradient(at 100% 50%, hsla(290,100%,60%,1) 0px, transparent 50%), radial-gradient(at 50% 0%, hsla(200,100%,70%,1) 0px, transparent 50%), radial-gradient(at 50% 100%, hsla(270,100%,50%,1) 0px, transparent 50%)",
	),
	grad(
		"mesh-warm",
		"Warm",
		"radial-gradient(at 20% 30%, hsla(35,100%,70%,1) 0px, transparent 50%), radial-gradient(at 80% 30%, hsla(15,100%,65%,1) 0px, transparent 50%), radial-gradient(at 50% 80%, hsla(45,100%,75%,1) 0px, transparent 50%)",
	),
];

/** 33 solid colors. */
export const REF_SOLIDS: BackgroundPreset[] = [
	solid("solid-white", "White", "#ffffff"),
	solid("solid-very_light_gray", "Very Light Gray", "#e5e5e5"),
	solid("solid-medium_light_gray", "Medium Light Gray", "#b3b3b3"),
	solid("solid-dark_gray", "Dark Gray", "#333333"),
	solid("solid-dark_charcoal", "Dark Charcoal", "#4a4a4a"),
	solid("solid-darker_charcoal", "Darker Charcoal", "#2a2a2a"),
	solid("solid-black", "Black", "#000000"),
	solid("solid-coral_red", "Coral Red", "#ff6b6b"),
	solid("solid-bright_lime", "Bright Lime", "#32cd32"),
	solid("solid-orange", "Orange", "#ffa500"),
	solid("solid-bright_yellow", "Bright Yellow", "#ffff00"),
	solid("solid-light_olive_green", "Light Olive Green", "#b8d433"),
	solid("solid-medium_green", "Medium Green", "#4caf50"),
	solid("solid-light_pastel_pink", "Light Pastel Pink", "#ffb3d9"),
	solid("solid-medium_green_2", "Medium Green 2", "#66bb6a"),
	solid("solid-light_pastel_pink_2", "Light Pastel Pink 2", "#ffc0e1"),
	solid("solid-light_peach", "Light Peach", "#ffd9b3"),
	solid("solid-light_beige", "Light Beige", "#fff5e6"),
	solid("solid-light_teal", "Light Teal", "#80d4c7"),
	solid("solid-light_yellow", "Light Yellow", "#fffacd"),
	solid("solid-light_mint_green", "Light Mint Green", "#c8f7c8"),
	solid("solid-light_teal_2", "Light Teal 2", "#7fcdcd"),
	solid("solid-medium_blue", "Medium Blue", "#4a90e2"),
	solid("solid-medium_purple_blue", "Medium Purple Blue", "#8b7ec8"),
	solid("solid-medium_blue_2", "Medium Blue 2", "#5dade2"),
	solid("solid-medium_purple_blue_2", "Medium Purple Blue 2", "#9b7ec8"),
	solid("solid-darker_purple", "Darker Purple", "#6a5acd"),
	solid("solid-bright_fuchsia", "Bright Fuchsia", "#ff00ff"),
	solid("solid-light_mint_green_2", "Light Mint Green 2", "#b3ffb3"),
	solid("solid-light_pastel_blue", "Light Pastel Blue", "#b3d9ff"),
	solid("solid-light_lavender", "Light Lavender", "#d4b3ff"),
	solid("solid-light_pastel_pink_3", "Light Pastel Pink 3", "#ffb3d9"),
	solid("solid-light_pastel_pink_4", "Light Pastel Pink 4", "#ffc0e1"),
];

/** 24 output aspect ratios (excludes the store-driven "custom" entry). */
export const REF_ASPECTS: AspectPreset[] = [
	{ id: "auto", label: "Auto", ratio: null },
	{ id: "1_1", label: "Square", ratio: 1 / 1 },
	{ id: "4_5", label: "Portrait", ratio: 4 / 5 },
	{ id: "9_16", label: "Story/Reel", ratio: 9 / 16 },
	{ id: "16_9", label: "Landscape", ratio: 16 / 9 },
	{ id: "3_4", label: "Portrait", ratio: 3 / 4 },
	{ id: "2_3", label: "Portrait", ratio: 2 / 3 },
	{ id: "og_image", label: "Open Graph", ratio: 40 / 21 },
	{ id: "twitter_banner", label: "Twitter Banner", ratio: 3 / 1 },
	{ id: "instagram_banner", label: "Instagram Banner", ratio: 1 / 1 },
	{ id: "youtube_banner", label: "YouTube Banner", ratio: 16 / 9 },
	{ id: "linkedin_banner", label: "LinkedIn Banner", ratio: 4 / 1 },
	{ id: "3_2", label: "Photo", ratio: 3 / 2 },
	{ id: "4_3", label: "Traditional", ratio: 4 / 3 },
	{ id: "5_4", label: "Photo", ratio: 5 / 4 },
	{ id: "16_10", label: "Widescreen", ratio: 16 / 10 },
	{ id: "youtube_thumbnail", label: "YouTube Thumbnail", ratio: 16 / 9 },
	{ id: "youtube_video", label: "YouTube Video", ratio: 16 / 9 },
	{ id: "pinterest_long", label: "Long Pin", ratio: 10 / 21 },
	{ id: "appstore_iphone65", label: 'iPhone 6.5"', ratio: 1284 / 2778 },
	{ id: "appstore_iphone55", label: 'iPhone 5.5"', ratio: 1242 / 2208 },
	{ id: "appstore_ipad", label: 'iPad Pro 12.9"', ratio: 2048 / 2732 },
	{ id: "appstore_iphone65_landscape", label: 'iPhone 6.5" Landscape', ratio: 2778 / 1284 },
	{ id: "appstore_iphone55_landscape", label: 'iPhone 5.5" Landscape', ratio: 2208 / 1242 },
	{ id: "appstore_ipad_landscape", label: 'iPad Pro 12.9" Landscape', ratio: 2732 / 2048 },
];
