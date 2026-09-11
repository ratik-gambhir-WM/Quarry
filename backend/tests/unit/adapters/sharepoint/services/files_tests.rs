use std::sync::Arc;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Existing {
    id: String,
    version: u8,
}

#[test]
fn diffs_added_modified_and_removed_files() {
    let new_files = vec![
        Existing {
            id: "same".into(),
            version: 2,
        },
        Existing {
            id: "added".into(),
            version: 1,
        },
    ];
    let existing_files = vec![
        Existing {
            id: "same".into(),
            version: 1,
        },
        Existing {
            id: "removed".into(),
            version: 1,
        },
    ];
    let options = DiffOptions {
        get_new_id: Arc::new(|file: &Existing| file.id.clone()),
        get_existing_id: Arc::new(|file: &Existing| file.id.clone()),
        should_update: Some(Arc::new(|new: &Existing, old: &Existing| {
            new.version != old.version
        })),
    };

    let result = diff_files(&new_files, &existing_files, &options);
    assert_eq!(result.added[0].id, "added");
    assert_eq!(result.modified[0].0.id, "same");
    assert_eq!(result.removed[0].id, "removed");
}
