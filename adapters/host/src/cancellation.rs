//! CLI signal delivery runs outside the async signal handler. The listener only
//! requests application cancellation; process supervision retains cleanup duty.
use madb_app::{ErrorKind, QueryError, Result, pipeline::PipelineControl};

#[cfg(unix)]
pub struct CancellationSignals {
    handle: signal_hook::iterator::Handle,
    listener: Option<std::thread::JoinHandle<()>>,
}
#[cfg(unix)]
impl CancellationSignals {
    pub fn install(control: &PipelineControl) -> Result<Self> {
        let control = control.clone();
        Self::install_action(move || {
            control.cancel();
        })
    }
    pub fn install_action(cancel: impl Fn() + Send + 'static) -> Result<Self> {
        use signal_hook::{
            consts::{SIGINT, SIGTERM},
            iterator::Signals,
        };
        let mut signals = Signals::new([SIGINT, SIGTERM]).map_err(error)?;
        let handle = signals.handle();
        let listener = std::thread::Builder::new()
            .name("madb-cancellation".to_owned())
            .spawn(move || {
                for _ in signals.forever() {
                    cancel();
                }
            })
            .map_err(error)?;
        Ok(Self {
            handle,
            listener: Some(listener),
        })
    }
}
#[cfg(unix)]
impl Drop for CancellationSignals {
    fn drop(&mut self) {
        self.handle.close();
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}
#[cfg(not(unix))]
pub struct CancellationSignals;
#[cfg(not(unix))]
impl CancellationSignals {
    pub fn install(_: &PipelineControl) -> Result<Self> {
        Err(error("signal cancellation requires a Unix host"))
    }
}
fn error(cause: impl std::fmt::Display) -> QueryError {
    QueryError::new(
        ErrorKind::Host,
        "HOST006",
        "cancellation",
        cause.to_string(),
    )
}
