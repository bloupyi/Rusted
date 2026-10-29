use rusted_launcher::{
    catalog::{self, Catalog},
    model::{self, Engine, Store},
};
use serde_json::{json, Value};

#[test]
fn offline_catalog_covers_releases_snapshots_and_special_builds() {
    let catalog = Catalog::bundled();
    let releases = catalog.ids(false);
    for id in ["26.1", "26.1.1", "26.1.2", "26.2", "26.3"] {
        assert!(releases.contains(&id.to_owned()), "Missing release: {id}");
    }
    let all = catalog.ids(true);
    for id in [
        "26.1-snapshot-1",
        "26.4-snapshot-6",
        "26w14a",
        "26.3-pre-1",
        "26.3-rc-1",
    ] {
        assert!(
            all.contains(&id.to_owned()),
            "Missing development build: {id}"
        );
        assert!(!releases.contains(&id.to_owned()));
    }
    assert!(!all
        .iter()
        .any(|id| id.starts_with("1.21") || id.starts_with("25w")));
    let d = tempfile::tempdir().unwrap();
    let store = Store::open(d.path().join("owned")).unwrap();
    assert_eq!(catalog::cached(&store).ids(true), all);
    std::fs::write(store.root.join("cache/versions.json"), b"INVALID_CACHE").unwrap();
    assert_eq!(catalog::cached(&store).ids(true), all);
    assert!(store.create("26.1 Java", Engine::Java, "26.1").is_ok());
    assert!(store.create("26.1 native", Engine::Native, "26.1").is_err());
}

#[test]
fn damaged_or_untrusted_catalog_does_not_become_a_download_source() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::open(d.path().join("owned")).unwrap();
    let catalog = Catalog::bundled();
    let valid = serde_json::to_value(&catalog).unwrap();
    for bad_url in [
        "http://piston-meta.mojang.com/version.json",
        "https://example.invalid/version.json",
        "https://user:password@piston-meta.mojang.com/version.json",
    ] {
        let mut value = valid.clone();
        value["versions"][0]["url"] = json!(bad_url);
        assert!(Catalog::parse(value.clone()).is_err());
        model::atomic_json(&store.root.join("cache/versions.json"), &value).unwrap();
        assert_eq!(catalog::cached(&store).ids(true), catalog.ids(true));
    }
    let mut duplicate = valid.clone();
    let entry = duplicate["versions"][0].clone();
    duplicate["versions"].as_array_mut().unwrap().push(entry);
    assert!(Catalog::parse(duplicate).is_err());
    let mut bad_hash = valid;
    bad_hash["versions"][0]["sha1"] = json!("invalid");
    assert!(Catalog::parse(bad_hash).is_err());
}

#[test]
fn selected_metadata_must_match_unobfuscated_modern_client() {
    let valid = json!({"id":"26.1","arguments":{"game":["--username","${auth_player_name}"]},"downloads":{"client":{"url":"https://example.invalid/client.jar"}}});
    catalog::validate_metadata("26.1", &valid).unwrap();
    assert!(catalog::validate_metadata("26.2", &valid).is_err());
    let mut obfuscated = valid.clone();
    obfuscated["downloads"]["client_mappings"] = json!({});
    assert!(catalog::validate_metadata("26.1", &obfuscated).is_err());
    let mut legacy = valid;
    legacy["arguments"] = Value::Null;
    assert!(catalog::validate_metadata("26.1", &legacy).is_err());
}
