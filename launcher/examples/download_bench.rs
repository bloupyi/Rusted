//! Cold public-asset sample comparing the previous 4-client strategy with the pooled queue.
//! Writes temporary test data only; never runs Minecraft or contacts an account service.
use rusted_launcher::{
    download::{self, Algorithm, Artifact},
    model, Task,
};
use serde_json::{json, Value};
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(), 2, "ASSET_INDEX RESULT_JSON");
    let index: Value = model::read_json(&PathBuf::from(&args[0])).unwrap();
    let mut sample = index["objects"]
        .as_object()
        .unwrap()
        .values()
        .filter(|v| (512..=8192).contains(&v["size"].as_u64().unwrap_or(0)))
        .map(|v| {
            (
                v["hash"].as_str().unwrap().to_owned(),
                v["size"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    sample.sort();
    sample.dedup();
    sample.truncate(256);
    assert_eq!(sample.len(), 256);
    let root = tempfile::tempdir().unwrap();
    let jobs = |name: &str| {
        sample
            .iter()
            .map(|(hash, size)| Artifact {
                url: format!(
                    "https://resources.download.minecraft.net/{}/{}",
                    &hash[..2],
                    hash
                ),
                path: root.path().join(name).join(hash),
                size: *size,
                hash: hash.clone(),
                algorithm: Algorithm::Sha1,
            })
            .collect::<Vec<_>>()
    };
    let legacy = jobs("old");
    let next = AtomicUsize::new(0);
    let before = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(a) = legacy.get(i) else { break };
                let agent = ureq::AgentBuilder::new()
                    .try_proxy_from_env(true)
                    .timeout_connect(Duration::from_secs(20))
                    .timeout_read(Duration::from_secs(15))
                    .build();
                let mut data = vec![];
                agent
                    .get(&a.url)
                    .call()
                    .unwrap()
                    .into_reader()
                    .take(8193)
                    .read_to_end(&mut data)
                    .unwrap();
                std::fs::create_dir_all(a.path.parent().unwrap()).unwrap();
                std::fs::write(&a.path, data).unwrap();
                assert!(download::valid(a));
            });
        }
    });
    let old = before.elapsed().as_secs_f64();
    let (tx, _rx) = mpsc::channel();
    let task = Task {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let before = Instant::now();
    download::queue(jobs("pooled"), 24, &task).unwrap();
    let new = before.elapsed().as_secs_f64();
    let result = json!({"sample_files":sample.len(),"bytes":sample.iter().map(|v|v.1).sum::<u64>(),"previous_four_fresh_agents_seconds":old,"pooled_twenty_four_workers_seconds":new,"speedup":old/new,"kind":"cold per-file cache, public Mojang CDN, Linux benchmark; not a Windows or Prism comparison"});
    model::atomic_json(&PathBuf::from(&args[1]), &result).unwrap();
    println!("{result}");
}
