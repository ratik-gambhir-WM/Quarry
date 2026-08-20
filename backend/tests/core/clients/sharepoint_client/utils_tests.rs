use super::*;

#[test]
fn parses_and_normalizes_sharepoint_paths() {
    assert_eq!(
        parse_sharepoint_folder_path(
            "https://company.sharepoint.com/:f:/r/sites/team/Shared%20Documents/My%20Folder?web=1"
        )
        .unwrap(),
        "/My Folder"
    );
    assert_eq!(normalize_path("///FOO/BAR///"), "foo/bar");
    assert!(is_path_excluded("some/path", Some(&["/SOME/PATH/".into()])));
    assert_eq!(normalize_file_extension("archive.tar.GZ"), "archive.tar.gz");
}

#[test]
fn drive_url_matches_encode_uri_component_semantics() {
    assert_eq!(
        build_drive_children_url("d1", "/My Folder/Sub", 100),
        "https://graph.microsoft.com/v1.0/drives/d1/root:%2FMy%20Folder%2FSub:/children?$top=100"
    );
}
