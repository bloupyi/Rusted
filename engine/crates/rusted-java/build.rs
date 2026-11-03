//! Compile our original Java API facades, never Minecraft or mod source code.
use std::{env, fs, io::Write, path::Path, process::Command};
fn collect(path: &Path, suffix: &str, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            collect(&p, suffix, out);
        } else if p.extension().is_some_and(|e| e == suffix) {
            out.push(p);
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=java");
    println!("cargo:rerun-if-env-changed=JAVA_HOME");
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let classes = out.join("classes");
    fs::create_dir_all(&classes).unwrap();
    let mut sources = Vec::new();
    collect(Path::new("java"), "java", &mut sources);
    sources.sort();
    let javac = env::var_os("JAVA_HOME")
        .map(std::path::PathBuf::from)
        .map(|p| {
            p.join(if cfg!(windows) {
                "bin/javac.exe"
            } else {
                "bin/javac"
            })
        })
        .unwrap_or_else(|| "javac".into());
    assert!(Command::new(javac)
        .args(["--release", "17", "-encoding", "UTF-8", "-d"])
        .arg(&classes)
        .args(&sources)
        .status()
        .expect("JDK 17+ is required to build Java mod compatibility")
        .success());
    let mut files = Vec::new();
    collect(&classes, "class", &mut files);
    files.sort();
    let mut jar = zip::ZipWriter::new(fs::File::create(out.join("rusted-java-api.jar")).unwrap());
    for path in files {
        jar.start_file(
            path.strip_prefix(&classes)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        jar.write_all(&fs::read(path).unwrap()).unwrap();
    }
    jar.finish().unwrap();
}
