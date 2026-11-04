use super::*;
#[test]
fn bounded_transport_inputs_and_normalization_cover_edges() {
    assert_eq!(name("  Platform\u{2003} team  ").unwrap(), "Platform team");
    assert!(name(&"é".repeat(100)).is_ok());
    for invalid in [
        String::new(),
        " ".into(),
        "x".repeat(101),
        "a\nb".into(),
        "a\0b".into(),
    ] {
        assert!(name(&invalid).is_err());
    }
    assert_eq!(
        paging(Paging {
            cursor: None,
            limit: None
        })
        .unwrap(),
        (String::new(), 30)
    );
    for limit in [0, 101, usize::MAX] {
        assert!(paging(Paging {
            cursor: None,
            limit: Some(limit)
        })
        .is_err());
    }
    for cursor in ["x".repeat(129), "newline\n".into()] {
        assert!(paging(Paging {
            cursor: Some(cursor),
            limit: None
        })
        .is_err());
    }
    for rev in [i64::MIN, -1, 0, 9_007_199_254_740_992, i64::MAX] {
        assert!(revision(rev).is_err());
    }
    assert!(revision(1).is_ok());
    assert!(revision(9_007_199_254_740_991).is_ok());
    assert!(user(0).is_err());
    assert!(user(1).is_ok());
    assert!(team("foreign").is_err());
    assert!(non_owner(DealRole::Owner).is_err());
    assert!(non_owner(DealRole::Viewer).is_ok());
}

#[test]
fn member_email_normalizes_and_rejects_usernames_and_malformed_inputs() {
    assert_eq!(
        member_email("  User+tag@WESTMONROE.COM  ").unwrap(),
        "user+tag@westmonroe.com"
    );
    assert!(member_email(&format!("{}@westmonroe.com", "a".repeat(305))).is_ok());
    for invalid in [
        "",
        "user",
        "@westmonroe.com",
        "user@localhost",
        "user@@westmonroe.com",
        "user name@westmonroe.com",
        "user\0@westmonroe.com",
        "user@westmonroe.\ncom",
    ] {
        assert!(member_email(invalid).is_err());
    }
    assert!(member_email(&format!("{}@westmonroe.com", "a".repeat(306))).is_err());
}
