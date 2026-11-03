//! Developer probe. The game uses this same runtime; no Java Minecraft main is run.
use std::path::PathBuf;
fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let java = args.next().ok_or("expected JAVA_HOME")?;
    if java == std::path::Path::new("--export-api") {
        let destination = args.next().ok_or("expected API jar destination")?;
        std::fs::write(destination, rusted_java::api_jar()).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let game = args.next().ok_or("expected disposable game directory")?;
    let cache = args.next().ok_or("expected disposable cache directory")?;
    let libraries: Vec<_> = args.collect();
    let (modules, errors) = rusted_java::discover(&game.join("mods"));
    for error in errors {
        println!("REJECTED {error}");
    }
    println!("MODULES {}", modules.len());
    if modules.is_empty() {
        return Err("No executable Fabric entrypoint JAR found".into());
    }
    let mut runtime = rusted_java::Runtime::load(&java, &game, &cache, &modules, &libraries)?;
    for time in 0..=60 {
        for action in runtime.tick(time, 17.5, true)? {
            match action {
                rusted_java::Action::Chat { module, text } => println!("CHAT [{module}] {text}"),
                rusted_java::Action::Error(error) => println!("ERROR {error}"),
            }
        }
    }
    println!("NATIVE HOST: 61 client ticks; no Java Minecraft simulation or renderer");
    Ok(())
}
