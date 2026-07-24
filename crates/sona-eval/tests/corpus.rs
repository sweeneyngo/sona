//! Corpus integration test. **`#[ignore]`d** so `cargo test` stays fast and
//! offline (ADR-0011) — the committed golden fixtures cover the always-on path.
//! After fetching the corpus, run it explicitly:
//!
//!     crates/sona-eval/scripts/fetch-vocadito.sh
//!     cargo test -p sona-eval -- --ignored

use sona_eval::vocadito;

#[test]
#[ignore = "requires the Vocadito corpus; run fetch-vocadito.sh then `cargo test -- --ignored`"]
fn loads_vocadito_corpus() {
    let root = vocadito::corpus_dir();
    let corpus = vocadito::discover(&root).expect("corpus should be present under SONA_VOCADITO_DIR");
    assert!(!corpus.tracks.is_empty(), "expected to discover tracks under {}", root.display());

    // Every discovered track loads into a Reference without error, and the
    // annotations we located are non-empty.
    for track in &corpus.tracks {
        let reference = vocadito::load_reference(track)
            .unwrap_or_else(|e| panic!("loading track {}: {e}", track.id));
        if track.f0.is_some() {
            assert!(!reference.f0.is_empty(), "track {} f0 parsed empty", track.id);
        }
        if track.notes_a1.is_some() || track.notes_a2.is_some() {
            assert!(!reference.notes.is_empty(), "track {} notes parsed empty", track.id);
        }
    }
}
