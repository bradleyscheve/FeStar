//! Background work that must not freeze the window: a period search, an AI
//! request. The app polls the task each frame until its result arrives.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, mpsc};

use vstar_core::Result;

/// Handed to the background closure so it can report progress and notice
/// when the user cancels.
#[derive(Clone)]
pub struct Handle {
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
    ctx: egui::Context,
}

impl Handle {
    /// Report progress (0 to 1). Returns `false` if the user cancelled.
    pub fn report(&self, fraction: f64) -> bool {
        self.progress.store(
            (fraction.clamp(0.0, 1.0) as f32).to_bits(),
            Ordering::Relaxed,
        );
        self.ctx.request_repaint();
        !self.is_cancelled()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

pub struct Task<T> {
    pub description: String,
    rx: mpsc::Receiver<Result<T>>,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
}

impl<T: Send + 'static> Task<T> {
    pub fn spawn(
        ctx: &egui::Context,
        description: impl Into<String>,
        work: impl FnOnce(&Handle) -> Result<T> + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = Handle {
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(AtomicU32::new(0f32.to_bits())),
            ctx: ctx.clone(),
        };
        let (cancel, progress) = (handle.cancel.clone(), handle.progress.clone());
        std::thread::spawn(move || {
            let result = work(&handle);
            let _ = tx.send(result);
            handle.ctx.request_repaint();
        });
        Self {
            description: description.into(),
            rx,
            cancel,
            progress,
        }
    }
}

impl<T> Task<T> {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn progress(&self) -> f32 {
        f32::from_bits(self.progress.load(Ordering::Relaxed))
    }

    /// The result, once the background thread has finished.
    pub fn poll(&self) -> Option<Result<T>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(vstar_core::Error::Network(
                "the background task stopped unexpectedly".into(),
            ))),
        }
    }
}
