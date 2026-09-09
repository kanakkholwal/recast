//! The edit-path cost the plan estimated (parse, map, evaluator rebuild on a real-sized document), measured.
//! Ignored by default: run with `cargo test -p recast-ffi-wasm --release -- bench_io --ignored --nocapture`.

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use recast_compositor::eval::{Evaluator, SourceGeometry};
    use recast_project::scene::{to_render_state, to_scene};
    use recast_project::{parse, serialize};

    /// A document the size of a real edited recording: cuts, zooms, annotations, captions, camera keys.
    fn document() -> String {
        let mut text = String::from(
            r##"<recast v="3" timebase="source-seconds" pad="4">
  <media id="rec" kind="video" src="media/recording.mp4" w="1920" h="1080" fps="60" dur="126.000"/>
  <media id="cam" kind="video" src="media/camera.mp4" offset="0.856"/>
  <track id="cur" kind="cursor" src="tracks/cursor.json" n="7420" ro="true"/>
  <timeline src="rec" in="0.500" out="120.000">
    <cuts>
"##,
        );
        for i in 0..12 {
            let at = 5.0 + f64::from(i) * 9.0;
            text.push_str(&format!(
                "      <cut id=\"c{i:02}\" at=\"{at:.3}\" dur=\"1.250\"/>\n"
            ));
        }
        text.push_str("    </cuts>\n  </timeline>\n  <background><solid color=\"#0f172a\"/></background>\n  <screen id=\"scr\"><zooms>\n");
        for i in 0..10 {
            let at = 3.0 + f64::from(i) * 11.0;
            text.push_str(&format!(
                "      <zoom id=\"z{i:02}\" at=\"{at:.3}\" dur=\"4.000\" scale=\"2\" cx=\"0.4\" cy=\"0.6\"/>\n"
            ));
        }
        text.push_str("  </zooms></screen>\n  <annotations>\n");
        for i in 0..8 {
            let at = 7.0 + f64::from(i) * 13.0;
            text.push_str(&format!(
                "    <rect id=\"a{i:02}\" at=\"{at:.3}\" dur=\"2.000\" x=\"0.1\" y=\"0.2\" w=\"0.3\" h=\"0.2\"/>\n"
            ));
        }
        text.push_str("  </annotations>\n</recast>\n");
        text
    }

    #[test]
    #[ignore = "a measurement, printed with --nocapture; run in release"]
    fn parse_map_and_evaluator_rebuild_on_a_real_sized_document() {
        let text = document();
        let doc = parse(&text).expect("the bench document parses");
        let canonical = serialize(&doc);
        let scene = to_scene(&doc).expect("maps");
        let geometry = SourceGeometry {
            width: 1920,
            height: 1080,
        };
        let rounds = 500;
        let started = Instant::now();
        for _ in 0..rounds {
            let doc = parse(&canonical).expect("parses");
            std::hint::black_box(doc);
        }
        let parse_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(rounds);
        let started = Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(to_scene(&doc).expect("maps"));
        }
        let map_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(rounds);
        let started = Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(Evaluator::new(&scene, geometry));
        }
        let eval_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(rounds);
        // The GUI edit path after step 5 phase 2: a patch of one field merged into the last state, migrated, evaluated.
        let state_json =
            serde_json::to_string(&to_render_state(&doc).expect("state")).expect("json");
        let mut patchable = crate::scene_io::PatchableState::default();
        patchable.remember(&state_json);
        let started = Instant::now();
        for i in 0..rounds {
            let state = patchable
                .apply(&format!("{{\"padding\": {}}}", i % 40))
                .expect("patch");
            let scene = recast_scene::migrate::to_scene(&state);
            std::hint::black_box(Evaluator::new(&scene, geometry));
        }
        let patch_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(rounds);
        println!(
            "document {} bytes: parse {parse_us:.0} us, map {map_us:.0} us, evaluator {eval_us:.0} us, edit path (map + evaluator) {:.0} us, GUI patch path (merge + migrate + evaluator) {patch_us:.0} us",
            canonical.len(),
            map_us + eval_us
        );
        assert!(
            patch_us < 5_000.0,
            "a patched edit must leave most of a frame for the draw"
        );
        assert!(
            map_us + eval_us < 20_000.0,
            "the edit path must stay well inside one frame"
        );
    }
}
