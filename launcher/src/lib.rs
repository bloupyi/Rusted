//! Original Rusted launcher. The game engine and preparer remain separately attributed.
pub mod auth;
pub mod catalog;
pub mod compat;
pub mod download;
pub mod install;
pub mod launch;
pub mod model;
pub mod mods;
pub mod packs;
pub type Result<T> = std::result::Result<T, String>;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

#[derive(Clone, Debug)]
pub enum Event {
    Status(String),
    Progress {
        done: u64,
        total: u64,
        label: String,
    },
    Log(String),
    AccountConnected(auth::Account),
    AccountFailed(String),
    DeviceCode {
        uri: String,
        code: String,
    },
    Done(String),
    Failed(String),
    SearchResults {
        request: u64,
        hits: Vec<mods::Hit>,
    },
    SearchFailed {
        request: u64,
        error: String,
    },
    VersionCatalog(catalog::Catalog),
    VersionCatalogFailed(String),
    GameStarted,
    GameStopped(i32),
}
#[derive(Clone)]
pub struct Task {
    pub events: mpsc::Sender<Event>,
    pub cancel: Arc<AtomicBool>,
}
impl Task {
    pub fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err("Operation annulee. Les fichiers deja verifies sont conserves.".into())
        } else {
            Ok(())
        }
    }
    pub fn send(&self, event: Event) {
        let _ = self.events.send(event);
    }
    pub fn status(&self, text: impl Into<String>) {
        self.send(Event::Status(text.into()));
    }
}
