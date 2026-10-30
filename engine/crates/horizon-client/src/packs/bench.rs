//! Opt-in CPU profiling with private prepared resources and an original test pack.
use super::*;

#[test]
fn passive_loader_archives_mount_resources_and_function_data_without_conversion()
-> crate::Result<()> {
    use std::io::Write;
    let base = std::env::temp_dir().join(format!("rusted-loader-mounts-{}", std::process::id()));
    let vanilla = base.join("vanilla");
    fs::create_dir_all(&vanilla)?;
    for (kind, metadata_path, metadata) in [
        (
            "fabric",
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"fixture","depends":{"minecraft":">=26.3 <26.4"}}"#,
        ),
        (
            "quilt",
            "quilt.mod.json",
            r#"{"quilt_loader":{"id":"fixture","depends":[{"id":"minecraft","versions":"26.3"}]}}"#,
        ),
        (
            "forge",
            "META-INF/mods.toml",
            "modLoader=\"lowcodefml\"\n[[mods]]\nmodId=\"fixture\"\n",
        ),
        (
            "neoforge",
            "META-INF/neoforge.mods.toml",
            "modLoader=\"lowcodefml\"\n[[mods]]\nmodId=\"fixture\"\n",
        ),
    ] {
        let directory = base.join(kind);
        fs::create_dir_all(&directory)?;
        let jar = directory.join("original.jar");
        let mut archive = zip::ZipWriter::new(fs::File::create(&jar)?);
        for (name, data) in [
            (metadata_path, metadata),
            (
                "assets/custom/lang/en_us.json",
                r#"{"fixture.name":"Copper"}"#,
            ),
            ("data/custom/function/greet.mcfunction", "say Copper\n"),
            (
                "data/minecraft/tags/function/load.json",
                r#"{"values":["custom:greet"]}"#,
            ),
        ] {
            archive.start_file(name, zip::write::SimpleFileOptions::default())?;
            archive.write_all(data.as_bytes())?;
        }
        archive.finish()?;
        let original = fs::read(&jar)?;
        let packs = mod_resources(&directory);
        assert_eq!(packs.len(), 1, "{kind}");
        let root = resource_root(&vanilla, &packs, &base.join(format!("cache-{kind}")))?;
        assert!(fs::read_to_string(root.join("assets/custom/lang/en_us.json"))?.contains("Copper"));
        let data = crate::datapack::mod_data(&directory);
        let store = crate::datapack::DataStore::load(&data.iter().collect::<Vec<_>>());
        assert!(store.errors.is_empty());
        assert!(store.functions.contains_key("custom:greet"));
        assert_eq!(store.function_tags["minecraft:load"][0].0, "custom:greet");
        assert_eq!(fs::read(&jar)?, original);
        fs::rename(&jar, jar.with_extension("jar.disabled"))?;
        assert!(mod_resources(&directory).is_empty());
        assert!(crate::datapack::mod_data(&directory).is_empty());
    }
    fs::remove_dir_all(base)?;
    Ok(())
}

#[test]
#[ignore = "requires RUSTED_PACK_BENCH_ROOT and RUSTED_PACK_BENCH_ZIP"]
fn profile_pack_reload() -> crate::Result<()> {
    use std::time::Instant;
    let vanilla = PathBuf::from(std::env::var("RUSTED_PACK_BENCH_ROOT")?);
    let zip = PathBuf::from(std::env::var("RUSTED_PACK_BENCH_ZIP")?);
    let scratch = std::env::temp_dir().join(format!("rusted-pack-profile-{}", std::process::id()));
    fs::create_dir_all(&scratch)?;
    let pack = server_pack(1, &zip).ok_or("invalid fixture pack")?;
    let assets = crate::assets::Assets::load(&vanilla)?;
    for iteration in 0..3 {
        let mut measured = BTreeMap::new();
        let start = Instant::now();
        let root = resource_root(&vanilla, std::slice::from_ref(&pack), &scratch)?;
        measured.insert("overlay_ms", start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        let render = assets.reload(&root)?;
        measured.insert("block_models_ms", start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        let atlas = crate::hud::HudAtlas::build_over(&root, &vanilla)?;
        measured.insert("gui_atlas_font_ms", start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        let items = crate::item_render::ItemRenderer::load_with_library(
            &root,
            render.model_library.clone(),
        )?;
        measured.insert(
            "item_definitions_ms",
            start.elapsed().as_secs_f64() * 1000.0,
        );
        println!(
            "{}",
            serde_json::json!({"iteration":iteration,"ms":measured,
            "sprites":atlas.sprites.len(),"items":items.models.definitions.len()})
        );
    }
    fs::remove_dir_all(scratch)?;
    Ok(())
}
