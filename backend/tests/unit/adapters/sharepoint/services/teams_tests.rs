use super::*;

#[test]
fn checks_team_and_encoded_channel_membership() {
    let teams = vec![TeamChannels {
        team_id: "team-1".into(),
        channels: vec!["channel%2Fone".into()],
    }];
    assert!(is_team_and_channel_member(
        &teams,
        "team-1",
        "channel%2Fone"
    ));
    assert!(!is_team_and_channel_member(&teams, "team-1", "other"));
}

#[test]
fn proxy_me_cache_requires_a_stable_scope() {
    let proxy = UserGraphProxyConfig::new("https://proxy.example.com");
    assert_eq!(user_graph_cache_key("teams", None, Some(&proxy)), None);

    let proxy = UserGraphProxyConfig {
        user_id: Some("user-1".into()),
        ..proxy
    };
    assert_eq!(
        user_graph_cache_key("teams", None, Some(&proxy)),
        Some("teams:proxy:user:user-1".into())
    );
}
