use crate::imp::polyfill;

use std::cmp;
use std::ops::RangeBounds;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Serial(u64);

impl Serial {
    #[inline(always)]
    pub fn next(self) -> Serial {
        Serial(self.0 + 1)
    }

    #[inline(always)]
    pub fn zero() -> Serial {
        Serial(0)
    }

    #[inline(always)]
    pub fn one() -> Serial {
        Serial(1)
    }

    #[inline(always)]
    pub fn get(self) -> u64 {
        self.0
    }

    /// Increments this serial and returns the new value
    #[inline(always)]
    pub fn increment(&mut self) -> Serial {
        *self = self.next();
        *self
    }
}

impl PartialEq<u64> for Serial {
    fn eq(&self, rhs: &u64) -> bool {
        self.0.eq(rhs)
    }
}

impl PartialOrd<u64> for Serial {
    fn partial_cmp(&self, rhs: &u64) -> Option<cmp::Ordering> {
        self.0.partial_cmp(rhs)
    }
}

#[derive(Debug)]
pub struct SerialQueue<T> {
    storage: Vec<(T, Serial)>,
}

impl<T> Default for SerialQueue<T> {
    fn default() -> SerialQueue<T> {
        SerialQueue { storage: Vec::new() }
    }
}

impl<T> SerialQueue<T> {
    pub fn new() -> SerialQueue<T> {
        SerialQueue::default()
    }

    /// Add the serial to the queue
    ///
    /// ## Panics
    ///
    /// Panics if the serial value is not greater than the head of the queue
    pub fn enqueue(&mut self, value: T, serial: Serial) {
        assert!(self.storage.is_empty() || self.storage.last().unwrap().1 <= serial);
        self.storage.push((value, serial));
    }

    /// Iterate up to and including the given serial
    pub fn iter_up_to(&self, serial: Serial) -> impl Iterator<Item = &(T, Serial)> {
        self.storage.iter().filter(move |item| item.1 <= serial).fuse()
    }

    /// Drain up to and including the given serial
    pub fn drain_up_to(&mut self, serial: Serial) -> impl Iterator<Item = (T, Serial)> + '_ {
        polyfill::drain_filter(&mut self.storage, move |item| item.1 <= serial)
    }

    pub fn drain<R: RangeBounds<usize>>(&mut self, range: R) -> impl Iterator<Item = (T, Serial)> + '_ {
        self.storage.drain(range)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.storage.len()
    }

    pub fn first(&self) -> Option<&(T, Serial)> {
        self.storage.first()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(T, Serial)> {
        self.storage.iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::imp::serial::{Serial, SerialQueue};

    #[test]
    fn serial_queue_iter_up_to() {
        let mut queue = SerialQueue::new();
        queue.enqueue("0", Serial(0));
        queue.enqueue("1", Serial(1));
        queue.enqueue("2", Serial(2));
        queue.enqueue("3", Serial(3));
        queue.enqueue("4", Serial(4));

        let a: Vec<_> = queue.iter_up_to(Serial(2)).map(|item| item.0).collect();

        assert_eq!(a, &["0", "1", "2"])
    }

    #[test]
    fn serial_queue_drain_up_to() {
        let mut queue = SerialQueue::new();
        queue.enqueue("0", Serial(0));
        queue.enqueue("1", Serial(1));
        queue.enqueue("2", Serial(2));
        queue.enqueue("3", Serial(3));
        queue.enqueue("4", Serial(4));

        queue.drain_up_to(Serial(2)).count();

        let a: Vec<_> = queue.iter_up_to(Serial(4)).map(|item| item.0).collect();

        assert_eq!(a, &["3", "4"])
    }
}
