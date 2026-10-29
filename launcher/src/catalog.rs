//! Official unobfuscated releases (26.1 onward), with an offline bootstrap catalogue.
use crate::{
    download,
    model::{self, Store},
    Result,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const BUNDLED: &str = include_str!("../versions.json");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: String,
    #[serde(rename = "releaseTime")]
    pub released: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalog {
    pub versions: Vec<Version>,
}

pub fn unobfuscated_id(id: &str) -> bool {
    // Mojang's year-based numbering started with 26.1, alongside removal of obfuscation.
    let Some(year) = id.get(..2) else {
        return false;
    };
    year.parse::<u32>().is_ok_and(|n| n >= 26)
        && matches!(id.as_bytes().get(2), Some(b'.' | b'w'))
        && id
            .get(3..)
            .is_some_and(|s| s.starts_with(|c: char| c.is_ascii_digit()))
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
}

impl Catalog {
    pub fn parse(value: Value) -> Result<Self> {
        let entries = value["versions"]
            .as_array()
            .ok_or("Catalogue Mojang invalide.")?;
        let mut versions = vec![];
        let mut seen = BTreeSet::new();
        for entry in entries {
            if !entry["id"].as_str().is_some_and(unobfuscated_id)
                || !matches!(entry["type"].as_str(), Some("release" | "snapshot"))
            {
                continue;
            }
            let v: Version =
                serde_json::from_value(entry.clone()).map_err(|_| "Version Mojang incomplete.")?;
            let url = url::Url::parse(&v.url).map_err(|_| "URL de version Mojang invalide.")?;
            if url.scheme() != "https"
                || url.host_str() != Some("piston-meta.mojang.com")
                || !url.username().is_empty()
                || url.password().is_some()
                || v.sha1.len() != 40
                || !v.sha1.bytes().all(|b| b.is_ascii_hexdigit())
                || v.released.len() < 20
                || !seen.insert(v.id.clone())
            {
                return Err(
                    "Catalogue Mojang incoherent : doublon, URL ou empreinte invalide.".into(),
                );
            }
            versions.push(v);
        }
        if versions.is_empty() {
            return Err("Aucune version deobfusquee dans le catalogue Mojang.".into());
        }
        // Use Mojang's newest-first ordering, rather than lexical sorting of version numbers.
        Ok(Self { versions })
    }
    pub fn ids(&self, snapshots: bool) -> Vec<String> {
        self.versions
            .iter()
            .filter(|v| snapshots || v.kind == "release")
            .map(|v| v.id.clone())
            .collect()
    }
    pub fn find(&self, id: &str) -> Option<&Version> {
        self.versions.iter().find(|v| v.id == id)
    }
    pub fn bundled() -> Self {
        Self::parse(serde_json::from_str(BUNDLED).expect("bundled Mojang catalogue JSON"))
            .expect("verified bundled Mojang catalogue")
    }
}

pub fn cached(store: &Store) -> Catalog {
    model::read_json::<Value>(&store.root.join("cache/versions.json"))
        .and_then(Catalog::parse)
        .unwrap_or_else(|_| Catalog::bundled())
}

pub fn refresh(store: &Store) -> Result<Catalog> {
    let catalogue = Catalog::parse(download::json(MANIFEST_URL)?)?;
    // Validate the complete response before replacing a usable cache.
    model::atomic_json(&store.root.join("cache/versions.json"), &catalogue)?;
    Ok(catalogue)
}

pub fn validate_metadata(id: &str, metadata: &Value) -> Result<()> {
    if metadata["id"] != id
        || !metadata["arguments"]["game"].is_array()
        || metadata["downloads"].get("client").is_none()
        || metadata["downloads"].get("client_mappings").is_some()
        || metadata["downloads"].get("server_mappings").is_some()
    {
        return Err(
            "Les metadonnees ne correspondent pas a une version officielle sans obfuscation."
                .into(),
        );
    }
    Ok(())
}
