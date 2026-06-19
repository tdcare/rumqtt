//! Patched quanta 0.12.6 for OHOS — uses std::time instead of libc::timespec.

use std::time::{SystemTime, UNIX_EPOCH, Duration as StdDuration, Instant as StdInstant};
use std::ops::{Add, AddAssign, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(StdInstant);

impl Instant {
    pub fn now() -> Instant { Instant(StdInstant::now()) }
    pub fn recent() -> Instant { Instant::now() }
    pub fn checked_sub(&self, dur: StdDuration) -> Option<Instant> {
        self.0.checked_sub(dur).map(Instant)
    }
    pub fn checked_add(&self, dur: StdDuration) -> Option<Instant> {
        self.0.checked_add(dur).map(Instant)
    }
    pub fn duration_since(&self, earlier: Instant) -> StdDuration {
        self.0.duration_since(earlier.0)
    }
}

impl Add<StdDuration> for Instant {
    type Output = Instant;
    fn add(self, dur: StdDuration) -> Instant { Instant(self.0 + dur) }
}

impl Sub<StdDuration> for Instant {
    type Output = Instant;
    fn sub(self, dur: StdDuration) -> Instant { Instant(self.0 - dur) }
}

impl Sub for Instant {
    type Output = StdDuration;
    fn sub(self, other: Instant) -> StdDuration { self.0.duration_since(other.0) }
}

impl AddAssign<StdDuration> for Instant {
    fn add_assign(&mut self, dur: StdDuration) { self.0 += dur; }
}

pub type Duration = StdDuration;

pub struct Clock;

impl Clock {
    pub fn new() -> Clock { Clock }
    pub fn now(&self) -> Instant { Instant::now() }
    pub fn recent(&self) -> Instant { Instant::now() }
    pub fn raw(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64
    }
    pub fn start(&self) -> Instant { Instant::now() }
    pub fn end(&self) -> Instant { Instant::now() }
    pub fn delta(&self, start: Instant, end: Instant) -> StdDuration {
        end.0.duration_since(start.0)
    }
}

impl Default for Clock {
    fn default() -> Self { Clock::new() }
}

pub fn recent() -> Instant { Instant::now() }
pub fn now() -> Instant { Instant::now() }

pub struct Mock;
impl Mock {
    pub fn new() -> Mock { Mock }
}
