//! Dump the camera list and one frame, for eyeballing orientation and colour.

use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "frame.bmp".into());
    let found = capturekit::cameras()?;
    for camera in &found {
        println!(
            "{} [{}]{}",
            camera.name,
            camera.id.0,
            if camera.is_default { " (default)" } else { "" }
        );
        for mode in camera.formats.iter().take(8) {
            println!(
                "    {}x{} {:?} @ {:?}",
                mode.width, mode.height, mode.pixel_format, mode.frame_rate
            );
        }
        println!("    ...{} modes", camera.formats.len());
    }
    let Some(camera) = found.into_iter().find(|c| c.is_default) else {
        println!("no camera attached");
        return Ok(());
    };

    let mut capture = capturekit::capturer(capturekit::Target::Camera(camera.id)).build()?;
    // Webcams auto-expose over the first few frames; the first is usually black.
    let mut frame = None;
    for _ in 0..15 {
        let next = capture.next_frame(Duration::from_secs(5))?;
        frame = Some((next.bytes().to_vec(), next.stride()));
    }
    let desc = capture.describe().clone();
    let (bytes, stride) = frame.ok_or("no frame")?;
    println!("{}x{} stride {stride}", desc.width, desc.height);
    std::fs::write(&out, bmp(&bytes, desc.width, desc.height, stride))?;
    println!("wrote {out}");
    capture.stop()?;
    Ok(())
}

/// A 32-bit top-down BMP, which needs a negative height in the header.
fn bmp(bytes: &[u8], width: u32, height: u32, stride: u32) -> Vec<u8> {
    let pixels = (stride * height) as usize;
    let mut out = Vec::with_capacity(122 + pixels);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(122 + pixels as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&122u32.to_le_bytes());
    out.extend_from_slice(&108u32.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&(-(height as i32)).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&(pixels as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    out.extend_from_slice(&0x00ff_0000u32.to_le_bytes());
    out.extend_from_slice(&0x0000_ff00u32.to_le_bytes());
    out.extend_from_slice(&0x0000_00ffu32.to_le_bytes());
    out.extend_from_slice(&0xff00_0000u32.to_le_bytes());
    out.extend_from_slice(b"BGRs");
    out.extend_from_slice(&[0u8; 48]);
    out.extend_from_slice(&bytes[..pixels.min(bytes.len())]);
    out
}
