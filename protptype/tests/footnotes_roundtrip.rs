use fractal::{DocElm, Footnote, Paragraph, Span};
use fractal::Style;
use uuid::Uuid;
mod test_utils;


#[test]
fn styles_and_annotations_roundtrip() {
    let paragraph = Paragraph {
        text: vec![
            Span {
                text: "Task: ".to_string(),
                styles: vec![fractal::Style::Bold],
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
        id: Uuid::new_v4().to_string(),
        title: "note-1".to_string(),
        body: vec![DocElm::Paragraph(Paragraph {
            text: vec![Span {
                text: "Remember to test checkboxes too.".to_string(),
                ..Span::new()
            }],
        })],
    }];

    test_utils::assert_roundtrip(test_utils::build_ir(vec![DocElm::Paragraph(paragraph)], Some(footnotes)));
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
            id: Uuid::new_v4().to_string(),
            title: "note-a".to_string(),
            body: vec![DocElm::Paragraph(Paragraph {
                text: vec![Span {
                    text: "First note".to_string(),
                    ..Span::new()
                }],
            })],
        },
        Footnote {
            id: Uuid::new_v4().to_string(),
            title: "note-b".to_string(),
            body: vec![DocElm::Paragraph(Paragraph {
                text: vec![Span {
                    text: "Second note".to_string(),
                    ..Span::new()
                }],
            })],
        },
    ];

    test_utils::assert_roundtrip(test_utils::build_ir(body, Some(footnotes)));
}
