use super::*;
use pptx_to_md::{
    ElementPosition, ListElement, ListItem, Run, TableCell, TableElement, TableRow, TextElement,
};

fn run(text: &str) -> Run {
    Run {
        text: text.to_string(),
        formatting: Default::default(),
    }
}

#[test]
fn extracts_text_from_slide_elements_in_visual_order() {
    let elements = vec![
        SlideElement::Text(
            TextElement {
                runs: vec![run("Subtitle")],
            },
            ElementPosition { x: 200, y: 200 },
        ),
        SlideElement::List(
            ListElement {
                items: vec![
                    ListItem {
                        level: 0,
                        is_ordered: false,
                        runs: vec![run("First bullet")],
                    },
                    ListItem {
                        level: 0,
                        is_ordered: false,
                        runs: vec![run("Second bullet")],
                    },
                ],
            },
            ElementPosition { x: 100, y: 300 },
        ),
        SlideElement::Table(
            TableElement {
                rows: vec![TableRow {
                    cells: vec![
                        TableCell {
                            runs: vec![run("Name")],
                        },
                        TableCell {
                            runs: vec![run("Amount")],
                        },
                    ],
                }],
            },
            ElementPosition { x: 100, y: 400 },
        ),
        SlideElement::Text(
            TextElement {
                runs: vec![run("Title")],
            },
            ElementPosition { x: 100, y: 100 },
        ),
    ];

    assert_eq!(
        slide_text(&elements),
        "Title\nSubtitle\nFirst bullet\nSecond bullet\nName\tAmount"
    );
}
