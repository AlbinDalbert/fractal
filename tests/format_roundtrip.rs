use fractal::{
    deserialize_ir, serialize_ir, DocElm, Footnote, Header, Image, IntermediateRep, Paragraph,
    Span, Style, FracFormatError,
};


#[test]
fn test_serialization_roundtrip() {
    // 1. Create the initial IR object, same as the one from main.rs
    let original_ir = IntermediateRep {
        size: 0, // Placeholder
        body: vec![
            DocElm::Header(Header {
                level: 1,
                text: Span {
                    text: "This is a Header".to_string(),
                    ..Span::new()
                },
            }),
            DocElm::Paragraph(Paragraph {
                text: vec![
                    Span {
                        text: "This is a simple paragraph.".to_string(),
                        ..Span::new()
                    },
                    Span {
                        text: " With a second span.".to_string(),
                        ..Span::new()
                    },
                ],
            }),
            DocElm::Paragraph(Paragraph {
                text: vec![
                    Span {
                        text: "This is a simple paragraph.".to_string(),
                        ..Span::new()
                    },
                    Span {
                        text: "This is ".to_string(),
                        ..Span::new()
                    },
                    Span {
                        text: "bold ".to_string(),
                        styles: vec![Style::Bold],
                        ..Span::new()
                    },
                    Span {
                        text: "and ".to_string(),
                        ..Span::new()
                    },
                    Span {
                        text: "italic".to_string(),
                        styles: vec![Style::Italic],
                        ..Span::new()
                    },
                    Span {
                        text: "and ".to_string(),
                        ..Span::new()
                    },
                    Span {
                        text: "both".to_string(),
                        styles: vec![Style::Bold, Style::Italic],
                        ..Span::new()
                    },
                ],
            }),
        ],
        count: 2,
        last_modified: Some(1678886400),
        author: Some("Gemini".to_string()),
        title: "Test Document".to_string(),
        checksum: None, // Placeholder
        tags: Some(vec!["test".to_string(), "example".to_string()]),
        footnotes: Some(vec![Footnote {
            title: "footnote1".to_string(),
            body: vec![DocElm::Paragraph(Paragraph {
                text: vec![Span {
                    text: "This is the content of the first footnote.".to_string(),
                    ..Span::new()
                }],
            })],
        }]),
    };

    // 2. Serialize the IR to bytes
    let bytes = serialize_ir(&original_ir).expect("Serialization failed");

    // 3. Deserialize the bytes back into an IR
    let deserialized_ir = deserialize_ir(&bytes).expect("Deserialization failed");

    // 4. Assert that the core content is identical.
    // We don't compare the whole structs because metadata like title, author, etc.,
    // are not currently saved in the file format. We only test what we expect to be saved.
    assert_eq!(original_ir.body, deserialized_ir.body);
    assert_eq!(original_ir.footnotes, deserialized_ir.footnotes);
}

fn build_ir(body: Vec<DocElm>, footnotes: Option<Vec<Footnote>>) -> IntermediateRep {
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

fn assert_roundtrip(ir: IntermediateRep) {
    let bytes = serialize_ir(&ir).expect("Serialization failed");
    let deserialized = deserialize_ir(&bytes).expect("Deserialization failed");
    assert_eq!(ir.body, deserialized.body);
    assert_eq!(ir.footnotes, deserialized.footnotes);
}

#[test]
fn styles_and_annotations_roundtrip() {
    let paragraph = Paragraph {
        text: vec![
            Span {
                text: "Task: ".to_string(),
                styles: vec![Style::Bold],
                ..Span::new()
            },
            Span {
                text: "Write serializer".to_string(),
                checkbox: Some(false),
                ..Span::new()
            },
            Span {
                text: " (see note)".to_string(),
                styles: vec![Style::Italic],
                footnote: Some("note-1".to_string()),
                ..Span::new()
            },
        ],
    };

    let footnotes = vec![Footnote {
        title: "note-1".to_string(),
        body: vec![DocElm::Paragraph(Paragraph {
            text: vec![Span {
                text: "Remember to test checkboxes too.".to_string(),
                ..Span::new()
            }],
        })],
    }];

    assert_roundtrip(build_ir(vec![DocElm::Paragraph(paragraph)], Some(footnotes)));
}

#[test]
fn link_and_highlight_styles_roundtrip() {
    let paragraph = Paragraph {
        text: vec![
            Span {
                text: "Docs available at ".to_string(),
                ..Span::new()
            },
            Span {
                text: "example".to_string(),
                styles: vec![Style::Link {
                    href: "https://example.com/docs".to_string(),
                }],
                ..Span::new()
            },
            Span {
                text: " and pay attention".to_string(),
                styles: vec![Style::Highlight],
                ..Span::new()
            },
        ],
    };

    assert_roundtrip(build_ir(vec![DocElm::Paragraph(paragraph)], None));
}

#[test]
fn multiple_footnotes_roundtrip() {
    let body = vec![DocElm::Paragraph(Paragraph {
        text: vec![Span {
            text: "Two notes follow".to_string(),
            ..Span::new()
        }],
    })];

    let footnotes = vec![
        Footnote {
            title: "note-a".to_string(),
            body: vec![DocElm::Paragraph(Paragraph {
                text: vec![Span {
                    text: "First note".to_string(),
                    ..Span::new()
                }],
            })],
        },
        Footnote {
            title: "note-b".to_string(),
            body: vec![DocElm::Paragraph(Paragraph {
                text: vec![Span {
                    text: "Second note".to_string(),
                    ..Span::new()
                }],
            })],
        },
    ];

    assert_roundtrip(build_ir(body, Some(footnotes)));
}

#[test]
fn unsupported_doc_elm_fails_serialization() {
    let ir = build_ir(
        vec![DocElm::Image(Image {
            title: "diagram".to_string(),
            source: "diagram.png".to_string(),
            decription: "not yet supported".to_string(),
        })],
        None,
    );

    let err = serialize_ir(&ir).expect_err("Image serialization should be rejected");
    assert!(matches!(err, FracFormatError::UnsupportedFeature(_)));
}
