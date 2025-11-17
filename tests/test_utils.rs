use fractal::{DocElm, Footnote, IntermediateRep, deserialize_ir, serialize_ir};

pub fn build_ir(body: Vec<DocElm>, footnotes: Option<Vec<Footnote>>) -> IntermediateRep {
    let count = body.len();
    IntermediateRep {
        size: 0,
        body,
        count,
        last_modified: Some(1_678_886_400),
        author: Some("Test Author".to_string()),
        title: "Test Document".to_string(),
        checksum: None,
        tags: Some(vec!["test".to_string(), "example".to_string()]),
        footnotes,
    }
}

pub fn assert_roundtrip(ir: IntermediateRep) {
    let bytes = serialize_ir(&ir).expect("Serialization failed");
    let deserialized = deserialize_ir(&bytes).expect("Deserialization failed");
    assert_eq!(ir.body, deserialized.body);
    assert_eq!(ir.footnotes, deserialized.footnotes);
}
