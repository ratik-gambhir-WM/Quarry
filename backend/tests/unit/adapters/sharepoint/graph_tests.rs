use super::*;

fn proxy() -> UserGraphProxyConfig {
    UserGraphProxyConfig::new("https://westmonroe-cloud.com")
}

#[test]
fn identifies_only_user_scoped_paths() {
    assert!(is_user_scoped_graph_path("/me/joinedTeams"));
    assert!(is_user_scoped_graph_path(
        "/users/alex@example.com/joinedTeams"
    ));
    assert!(!is_user_scoped_graph_path("/users"));
    assert!(!is_user_scoped_graph_path("/drives/drive-id/root"));
}

#[test]
fn maps_user_paths_through_proxy() {
    let proxy = proxy();
    assert_eq!(
        build_graph_url(
            "/me/joinedTeams",
            GraphRequestContext {
                token: None,
                user_graph_proxy: Some(&proxy),
            },
            GraphRequestOptions::default(),
        ),
        "https://westmonroe-cloud.com/users/me/graph/joinedTeams"
    );

    let configured = UserGraphProxyConfig {
        base_url: "https://westmonroe-cloud.com/".into(),
        user_id: Some("alex@example.com".into()),
        cache_key: None,
    };
    assert_eq!(
        build_graph_url(
            "/me/photos/48x48/$value",
            GraphRequestContext {
                token: None,
                user_graph_proxy: Some(&configured),
            },
            GraphRequestOptions::default(),
        ),
        "https://westmonroe-cloud.com/users/alex%40example.com/graph/photos/48x48/$value"
    );
}

#[test]
fn keeps_non_user_paths_on_direct_graph() {
    let proxy = proxy();
    assert_eq!(
        build_graph_url(
            "/drives/drive-id/root",
            GraphRequestContext {
                token: Some("Bearer token"),
                user_graph_proxy: Some(&proxy),
            },
            GraphRequestOptions::default(),
        ),
        "https://graph.microsoft.com/v1.0/drives/drive-id/root"
    );
}
