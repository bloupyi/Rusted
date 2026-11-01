//! Dev-only end-to-end probe: classify a loader JAR, load its native code, draw its HUD.
use std::path::PathBuf;
fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let mods_dir = args.next().ok_or("expected mods directory")?;
    let cache = args.next().ok_or("expected disposable cache directory")?;
    let paths = rusted_platform::native::discover(&mods_dir, &cache)?;
    if paths.len() != 1 {
        return Err(format!("Expected one native port, got {}", paths.len()));
    }
    let mut mods = horizon_mods::Mods::load(&paths, |_| true, |_| false);
    if !mods.is_running() {
        return Err(format!(
            "Native port did not start: {:?}",
            mods.entries()[0].status
        ));
    }
    for tick in 0..60 {
        mods.tick(tick);
        mods.frame(1.0 / 60.0);
    }
    let panels = mods.corner_panels().count() + mods.panels().count();
    if panels == 0 {
        return Err("No HUD output from native port".into());
    }
    println!("Native companion running: 60 ticks, 60 frames, {panels} HUD panels; no JVM");
    Ok(())
}
