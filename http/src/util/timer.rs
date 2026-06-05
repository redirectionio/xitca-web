use core::{
    pin::Pin,
    task::{Context, Poll, ready},
};

use pin_project_lite::pin_project;
use tokio::time::{Instant, Sleep, sleep_until};
use xitca_service::shutdown::{ShutdownListener, ShutdownState};

pub(crate) trait Timeout: Sized {
    fn timeout(self, timer: Pin<&mut KeepAlive>) -> TimeoutFuture<'_, Self>;
}

impl<F> Timeout for F
where
    F: Future,
{
    fn timeout(self, timer: Pin<&mut KeepAlive>) -> TimeoutFuture<'_, Self> {
        TimeoutFuture { fut: self, timer }
    }
}

pin_project! {
    pub(crate) struct TimeoutFuture<'a, F> {
        #[pin]
        fut: F,
        timer: Pin<&'a mut KeepAlive>
    }
}

impl<F: Future> Future for TimeoutFuture<'_, F> {
    type Output = Result<F::Output, KeepAliveOutput>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.fut.poll(cx) {
            Poll::Ready(res) => Poll::Ready(Ok(res)),
            Poll::Pending => this.timer.as_mut().poll(cx).map(Err),
        }
    }
}

pin_project! {
    /// A timer lazily reset the deadline after each successful poll(previous deadline met).
    ///
    /// This timer would optimistically assume deadline is not likely to be reached often.
    /// It has little cost inserting a new deadline and additional cost when previous
    /// deadline is met and the lazy reset happen with new deadline.
    pub struct KeepAlive {
        #[pin]
        timer: Sleep,
        deadline: Instant,
        // ShutdownState is Unpin (Pin<Box<T>> is Unpin), so no #[pin] needed.
        shutdown: ShutdownState,
    }
}

impl KeepAlive {
    #[inline]
    pub fn new(deadline: Instant, shutdown: Option<impl ShutdownListener + 'static>) -> Self {
        Self {
            timer: sleep_until(deadline),
            deadline,
            shutdown: shutdown.into(),
        }
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    #[inline]
    pub fn update(self: Pin<&mut Self>, deadline: Instant) {
        *self.project().deadline = deadline;
    }

    #[inline]
    pub fn reset(self: Pin<&mut Self>) {
        let this = self.project();
        this.timer.reset(*this.deadline)
    }

    fn is_expired(&self) -> bool {
        self.timer.deadline() >= self.deadline
    }
}

impl Future for KeepAlive {
    type Output = KeepAliveOutput;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();

        if this.shutdown.poll_cancelled(cx) {
            return Poll::Ready(KeepAliveOutput::Cancel);
        }

        ready!(this.timer.poll(cx));

        if self.is_expired() {
            Poll::Ready(KeepAliveOutput::Expire)
        } else {
            self.as_mut().reset();
            self.poll(cx)
        }
    }
}

/// return type of timer when it's finished
pub enum KeepAliveOutput {
    /// Timer is canceled by foreign input (e.g. a shutdown signal)
    Cancel,
    /// Timer is expired
    Expire,
}
