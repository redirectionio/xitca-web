use super::Command;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use xitca_io::net::Listener;

#[derive(Clone)]
pub struct ServerHandle {
    pub(super) tx: UnboundedSender<Command>,
    pub(super) listeners: Vec<Arc<Listener>>,
}

impl ServerHandle {
    /// Stop xitca-server with graceful flag.
    pub fn stop(self, graceful: bool) -> Vec<Arc<Listener>> {
        let cmd = if graceful {
            Command::GracefulStop
        } else {
            Command::ForceStop
        };

        let _ = self.tx.send(cmd);

        self.listeners
    }
}
