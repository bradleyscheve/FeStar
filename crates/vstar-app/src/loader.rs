//! Runs data loads on a background thread so the window stays responsive.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

use vstar_core::http::ReqwestClient;
use vstar_core::sources::{aavso, csv_import, gaia};
use vstar_core::{Dataset, Result, StarQuery};

pub enum Request {
    Aavso(StarQuery),
    Gaia(String),
    File { path: PathBuf, source_label: String },
}

pub struct Job {
    pub description: String,
    /// The data came from an online database, so the star can be
    /// re-fetched by name later.
    pub from_database: bool,
    rx: mpsc::Receiver<Result<Dataset>>,
    cancel: Arc<AtomicBool>,
    loaded: Arc<AtomicUsize>,
}

impl Job {
    pub fn start(request: Request, ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let loaded = Arc::new(AtomicUsize::new(0));
        let description = match &request {
            Request::Aavso(q) => format!("Loading {} from AAVSO…", q.name),
            Request::Gaia(name) => format!("Loading {name} from the Gaia archive…"),
            Request::File { path, .. } => {
                format!(
                    "Reading {}…",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )
            }
        };

        let from_database = matches!(request, Request::Aavso(_) | Request::Gaia(_));
        let (cancel2, loaded2) = (cancel.clone(), loaded.clone());
        std::thread::spawn(move || {
            let result = match request {
                Request::Aavso(query) => ReqwestClient::new().and_then(|http| {
                    let progress = |n: usize, _total: Option<u64>| {
                        loaded2.store(n, Ordering::Relaxed);
                        ctx.request_repaint();
                        !cancel2.load(Ordering::Relaxed)
                    };
                    aavso::load(&http, &query, &progress)
                }),
                Request::Gaia(name) => {
                    ReqwestClient::new().and_then(|http| gaia::load(&http, &name))
                }
                Request::File { path, source_label } => csv_import::load_file(&path, &source_label),
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });

        Self {
            description,
            from_database,
            rx,
            cancel,
            loaded,
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn loaded(&self) -> usize {
        self.loaded.load(Ordering::Relaxed)
    }

    /// The result, once the background thread has finished.
    pub fn poll(&self) -> Option<Result<Dataset>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(vstar_core::Error::Network(
                "the loader stopped unexpectedly".into(),
            ))),
        }
    }
}
