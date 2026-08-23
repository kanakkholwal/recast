import { describe, expect, it } from "vitest";
import { crc32, zipStore } from "./zip";

const bytes = (s: string) => new TextEncoder().encode(s);

describe("crc32", () => {
	it("matches known checksums", () => {
		expect(crc32(bytes(""))).toBe(0);
		// Well-known CRC-32 of the ASCII string "hello".
		expect(crc32(bytes("hello"))).toBe(0x3610a686);
	});
});

describe("zipStore", () => {
	it("writes a valid EOCD with the right entry count", async () => {
		const blob = zipStore([
			{ name: "a.txt", data: bytes("alpha") },
			{ name: "b.txt", data: bytes("beta") },
		]);
		expect(blob.type).toBe("application/zip");
		const buf = new Uint8Array(await blob.arrayBuffer());
		// Local file header signature at the very start.
		const head = new DataView(buf.buffer);
		expect(head.getUint32(0, true)).toBe(0x04034b50);
		// End-of-central-directory record is the last 22 bytes.
		const eocd = new DataView(buf.buffer, buf.length - 22);
		expect(eocd.getUint32(0, true)).toBe(0x06054b50);
		expect(eocd.getUint16(10, true)).toBe(2); // total entries
	});
});
