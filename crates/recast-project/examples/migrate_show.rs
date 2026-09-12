//! Migrates one bundle into a temp directory and prints the document; a hand tool, not shipped.
fn main() {
    let src = std::env::args().nth(1).expect("bundle path");
    let dest = std::env::temp_dir().join("recast-migrate-show.recast");
    let _ = std::fs::remove_dir_all(&dest);
    let report = recast_project::migrate::migrate(
        std::path::Path::new(&src),
        &dest,
        recast_project::migrate::Options::default(),
    )
    .expect("migrate");
    println!(
        "{}",
        std::fs::read_to_string(dest.join(recast_project::layout::DOCUMENT)).expect("doc")
    );
    println!("warnings: {:?}", report.warnings);
    for entry in std::fs::read_dir(&dest).expect("dir").flatten() {
        println!("  {}", entry.path().display());
    }
}
