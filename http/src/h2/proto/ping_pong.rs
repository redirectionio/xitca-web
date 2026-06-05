use core::{pin::Pin, time::Duration};

use std::io;

use crate::{
    date::{DateTime, DateTimeHandle},
    util::timer::{KeepAlive, KeepAliveOutput},
};

use super::flow::FlowControlLock;

pub(crate) struct PingPong<'a> {
    timer: Pin<&'a mut KeepAlive>,
    flow: &'a FlowControlLock,
    date: &'a DateTimeHandle,
    ka_dur: Duration,
}

impl<'a> PingPong<'a> {
    pub(crate) fn new(
        timer: Pin<&'a mut KeepAlive>,
        flow: &'a FlowControlLock,
        date: &'a DateTimeHandle,
        ka_dur: Duration,
    ) -> Self {
        Self {
            timer,
            flow,
            date,
            ka_dur,
        }
    }

    pub(crate) async fn tick(&mut self) -> io::Result<()> {
        if let KeepAliveOutput::Cancel = self.timer.as_mut().await {
            return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "shutdown"));
        }

        self.flow.borrow_mut().try_set_pending_ping()?;

        self.timer.as_mut().update(self.date.now() + self.ka_dur);

        Ok(())
    }
}
