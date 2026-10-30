use super::*;
use std::io::Write;

fn fixture(path: &Path, metadata: &str, extra: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, data) in [
        ("fabric.mod.json", metadata.as_bytes()),
        ("assets/example/lang/en_us.json", b"{}".as_slice()),
    ]
    .into_iter()
    .chain(extra.iter().copied())
    {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn code_hooks_dependencies_unsupported_data_and_portability_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let jar = dir.path().join("fixture.jar");
    let valid = r#"{"schemaVersion":1,"id":"own_fixture","depends":{"minecraft":">=26.3 <26.4"}}"#;
    for extra in [
        ("example/Hook.class", b"OWN_NOT_BYTECODE".as_slice()),
        ("nested.jar", b"OWN_NOT_JAR".as_slice()),
        ("../escape.json", b"{}".as_slice()),
        ("assets/example/lang/CON.json", b"{}".as_slice()),
        ("assets/example/lang/EN_US.json", b"{}".as_slice()),
        ("data/example/recipe/unsupported.json", b"{}".as_slice()),
    ] {
        fixture(&jar, valid, &[extra]);
        let before = fs::read(&jar).unwrap();
        assert!(inspect(&jar).is_err(), "{}", extra.0);
        assert_eq!(fs::read(&jar).unwrap(), before);
    }
    for metadata in [
        r#"{"schemaVersion":1,"id":"own_fixture","entrypoints":{"client":["own.Hook"]}}"#,
        r#"{"schemaVersion":1,"id":"own_fixture","mixins":["own.json"]}"#,
        r#"{"schemaVersion":1,"id":"own_fixture","depends":{"minecraft":"<26.3"}}"#,
        r#"{"schemaVersion":1,"id":"own_fixture","depends":{"fabric-api":"*"}}"#,
    ] {
        fixture(&jar, metadata, &[]);
        assert!(inspect(&jar).is_err());
    }
    fixture(&jar, valid, &[]);
    assert!(inspect(&jar).is_ok());
    fs::copy(&jar, dir.path().join("duplicate.jar")).unwrap();
    let report = crate::scan(dir.path());
    assert_eq!(report.mods.len(), 2);
    assert!(report.mods.iter().all(|m| !m.native_compatible));
    assert!(discover(dir.path()).is_empty());
}
