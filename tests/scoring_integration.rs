//! End-to-end check of the scoring pipeline against the example fixture.

use dartscore::{score_detection, Detection, Ring};

#[test]
fn scores_example_fixture() {
    let raw = std::fs::read_to_string("examples/detection.json").expect("read fixture");
    let det: Detection = serde_json::from_str(&raw).expect("parse fixture");

    let scores = score_detection(&det).expect("score detection");
    assert_eq!(scores.len(), 3);

    assert_eq!(scores[0].ring, Ring::InnerBull);
    assert_eq!(scores[0].value, 50);

    assert_eq!(scores[1].ring, Ring::Treble);
    assert_eq!(scores[1].segment, 20);
    assert_eq!(scores[1].value, 60);

    assert_eq!(scores[2].ring, Ring::Single);
    assert_eq!(scores[2].segment, 6);

    let total: u32 = scores.iter().map(|s| s.value).sum();
    assert_eq!(total, 116);
}
