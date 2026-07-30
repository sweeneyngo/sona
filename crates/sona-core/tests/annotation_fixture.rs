//! A hand-written example annotation JSON must validate against the Serde
//! types, and re-serialize to an equal `Annotation` value.

use sona_core::{Annotation, Producer, SCHEMA_VERSION};

const EXAMPLE: &str = include_str!("fixtures/annotation-example.json");

#[test]
fn hand_written_example_validates() {
    let annotation: Annotation =
        serde_json::from_str(EXAMPLE).expect("hand-written example JSON should deserialize");

    assert_eq!(annotation.version, SCHEMA_VERSION);
    assert_eq!(annotation.producer, Producer::Pyin);
    assert_eq!(annotation.notes.len(), 3);
    // Confidence descends across the excerpt — the last note is the flag-worthy
    // one (near the noise floor), exactly what the review surface surfaces.
    assert!(annotation.notes[0].confidence > annotation.notes[2].confidence);
    assert!(!annotation.f0_contour.is_empty());
    assert!(!annotation.energy_envelope.is_empty());
}

#[test]
fn example_re_serializes_to_equal_value() {
    let annotation: Annotation = serde_json::from_str(EXAMPLE).unwrap();
    let reencoded = serde_json::to_string(&annotation).unwrap();
    let decoded_again: Annotation = serde_json::from_str(&reencoded).unwrap();
    assert_eq!(annotation, decoded_again);
}
