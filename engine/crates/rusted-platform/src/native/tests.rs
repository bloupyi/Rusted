use super::*;
use std::io::Write;
fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, body) in files {
        out.start_file(*path, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(body).unwrap();
    }
    out.finish().unwrap().into_inner()
}
fn package() -> Vec<u8> {
    zip(&[
        (
            "horizon-mod.toml",
            b"id='test_port'\nname='Test port'\nversion='1.0'\napi='0.1'\n",
        ),
        ("mod.wasm", b"\0asm\x0d\0\x01\0"),
    ])
}
fn jar(
    metadata: (&str, &[u8]),
    package: &[u8],
    change: impl FnOnce(&mut serde_json::Value),
) -> Vec<u8> {
    let mut d = serde_json::json!({"schema":1,"minecraft":"26.3","id":"test_port",
        "package":"META-INF/rusted/port.hmod","sha256":format!("{:x}", Sha256::digest(package))});
    change(&mut d);
    zip(&[
        metadata,
        (MARKER, &serde_json::to_vec(&d).unwrap()),
        ("META-INF/rusted/port.hmod", package),
        ("example/JavaOnly.class", b"not executed"),
    ])
}
const FABRIC: (&str, &[u8]) = ("fabric.mod.json", br#"{"schemaVersion":1,"id":"test_port","entrypoints":{"client":["example.JavaOnly"]},"mixins":["a.json"]}"#);
#[test]
fn original_jar_requires_a_port_but_each_loader_can_supply_one() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("example.jar");
    fs::write(
        &file,
        zip(&[FABRIC, ("example/JavaOnly.class", b"not executed")]),
    )
    .unwrap();
    assert!(inspect(&file).unwrap().is_none());
    assert!(crate::native_id(&file).is_err());
    let metadata: [(&str, &[u8]); 4] = [
        FABRIC,
        (
            "quilt.mod.json",
            br#"{"quilt_loader":{"id":"test_port","entrypoints":{"init":["example.JavaOnly"]}}}"#,
        ),
        (
            "META-INF/mods.toml",
            b"modLoader='javafml'\n[[mods]]\nmodId='test_port'\n",
        ),
        (
            "META-INF/neoforge.mods.toml",
            b"modLoader='javafml'\n[[mods]]\nmodId='test_port'\n",
        ),
    ];
    for m in metadata {
        fs::write(&file, jar(m, &package(), |_| {})).unwrap();
        assert_eq!(crate::native_id(&file).unwrap(), "test_port");
        let report = crate::scan(dir.path());
        assert!(
            report.mods[0].native_compatible,
            "{}",
            report.mods[0].diagnostic
        );
        assert!(report.mods[0].diagnostic.contains("Java is not executed"));
    }
}
#[test]
fn bad_identity_checksum_and_schema_do_not_fall_back_to_java() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("example.jar");
    for (key, value) in [
        ("id", serde_json::json!("other")),
        ("sha256", serde_json::json!("bad")),
        ("schema", serde_json::json!(2)),
        ("package", serde_json::json!("../bad.hmod")),
    ] {
        fs::write(&file, jar(FABRIC, &package(), |d| d[key] = value)).unwrap();
        assert!(inspect(&file).is_err());
        assert!(!crate::scan(dir.path()).mods[0].native_compatible);
    }
    let bad = zip(&[("../horizon-mod.toml", b"bad"), ("mod.wasm", b"bad")]);
    fs::write(&file, jar(FABRIC, &bad, |_| {})).unwrap();
    assert!(inspect(&file).is_err());
    fs::write(
        &file,
        jar(
            ("META-INF/mods.toml", b"modLoader='javafml'"),
            &package(),
            |_| {},
        ),
    )
    .unwrap();
    assert!(inspect(&file).is_err()); // malformed TOML metadata must not panic
}
#[test]
fn disabled_removed_and_duplicate_jars_never_leave_an_active_cached_port() {
    let dir = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let file = dir.path().join("example.jar");
    fs::write(&file, jar(FABRIC, &package(), |_| {})).unwrap();
    let paths = discover(dir.path(), cache.path()).unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(fs::read(&paths[0]).unwrap(), package());
    fs::write(&paths[0], b"corrupt").unwrap();
    discover(dir.path(), cache.path()).unwrap();
    assert_eq!(fs::read(&paths[0]).unwrap(), package());
    fs::copy(&file, dir.path().join("duplicate.jar")).unwrap();
    assert!(discover(dir.path(), cache.path()).unwrap().is_empty());
    fs::remove_file(dir.path().join("duplicate.jar")).unwrap();
    fs::rename(&file, dir.path().join("example.jar.disabled")).unwrap();
    assert!(discover(dir.path(), cache.path()).unwrap().is_empty());
    assert!(paths[0].exists()); // cached bytes are not independently discovered
}

#[test]
fn four_loader_formats_can_coexist_in_one_native_inventory() {
    let dir = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let metadata = [
        FABRIC,
        (
            "quilt.mod.json",
            br#"{"quilt_loader":{"id":"test_port"}}"# as &[u8],
        ),
        ("META-INF/mods.toml", b"[[mods]]\nmodId='test_port'\n"),
        (
            "META-INF/neoforge.mods.toml",
            b"[[mods]]\nmodId='test_port'\n",
        ),
    ];
    for (index, (path, bytes)) in metadata.into_iter().enumerate() {
        let id = format!("port_{index}");
        let manifest = format!("id='{id}'\nname='Port {index}'\nversion='1'\napi='0.1'\n");
        let package = zip(&[
            ("horizon-mod.toml", manifest.as_bytes()),
            ("mod.wasm", b"\0asm\x0d\0\x01\0"),
        ]);
        let metadata = std::str::from_utf8(bytes)
            .unwrap()
            .replace("test_port", &id);
        let body = jar((path, metadata.as_bytes()), &package, |v| {
            v["id"] = serde_json::json!(id)
        });
        fs::write(dir.path().join(format!("mod-{index}.jar")), body).unwrap();
    }
    let report = crate::scan(dir.path());
    assert_eq!(report.mods.len(), 4);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(report.mods.iter().all(|m| m.native_compatible));
    assert_eq!(discover(dir.path(), cache.path()).unwrap().len(), 4);
}
