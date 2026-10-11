//! Events (EVT-1): cursor readers, exactly-once per reader, two-update
//! lifetime; new readers see only the future. EVT-7: update granularity is
//! documented per build — windowed = per frame, headless = per step.
use crate::world::Entity;

#[derive(Clone, Copy, Debug)]
pub struct CollisionEvent {
    pub mover: Entity,
    pub obstacle: Entity,
}

pub struct Reader {
    next: u64,
}

pub struct Events<T> {
    buf: Vec<(u64, u64, T)>, // (born_update, seq, event)
    seq: u64,
    updates: u64,
}

impl<T: 'static> Events<T> {
    pub fn new() -> Self {
        Self { buf: vec![], seq: 0, updates: 0 }
    }

    pub fn send(&mut self, e: T) {
        let born = self.updates;
        self.buf.push((born, self.seq, e));
        self.seq += 1;
    }

    /// Events live exactly two updates (EVT-1).
    pub fn update(&mut self) {
        self.updates += 1;
        self.buf.retain(|&(born, _, _)| self.updates <= born + 2);
    }

    /// New readers start at the current frontier — no retro-delivery.
    pub fn new_reader(&self) -> Reader {
        Reader { next: self.seq }
    }

    /// Exactly-once per reader, ascending sequence order.
    pub fn read(&mut self, r: &mut Reader) -> std::vec::IntoIter<T> {
        let mut taken = Vec::new();
        let mut kept = Vec::with_capacity(self.buf.len());
        for (born, seq, e) in self.buf.drain(..) {
            if seq >= r.next {
                taken.push(e);
            } else {
                kept.push((born, seq, e));
            }
        }
        self.buf = kept;
        r.next = self.seq;
        taken.into_iter()
    }
}

/// Bus + default cursor (the single-consumer pattern used by the demo).
pub struct Channel<T> {
    bus: Events<T>,
    reader: Reader,
}

impl<T: 'static> Channel<T> {
    pub fn new() -> Self {
        let mut b = Events::new();
        let r = b.new_reader();
        Self { bus: b, reader: r }
    }
    pub fn send(&mut self, e: T) {
        self.bus.send(e);
    }
    pub fn update(&mut self) {
        self.bus.update();
    }
    pub fn read(&mut self) -> std::vec::IntoIter<T> {
        self.bus.read(&mut self.reader)
    }
}

impl<T: 'static> Default for Channel<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [T] EVT-1 — exactly-once, ascending order.
    #[test]
    fn exactly_once_and_order() {
        let mut ev = Events::new();
        let mut r = ev.new_reader();
        ev.send(1);
        ev.send(2);
        ev.update();
        assert_eq!(ev.read(&mut r).collect::<Vec<_>>(), vec![1, 2]);
        ev.update();
        assert_eq!(ev.read(&mut r).count(), 0);
    }

    /// [T] EVT-1 — a reader that skips a window still catches its events.
    #[test]
    fn two_update_lifetime() {
        let mut ev = Events::new();
        let mut r = ev.new_reader();
        ev.send(10);
        ev.update();
        ev.update();
        assert_eq!(ev.read(&mut r).collect::<Vec<_>>(), vec![10]);
        ev.update();
        assert_eq!(ev.read(&mut r).count(), 0);
    }

    /// [T] EVT-1 — late readers see only the future.
    #[test]
    fn late_reader_future_only() {
        let mut ev = Events::new();
        ev.send(1);
        ev.update();
        let late = ev.new_reader();
        ev.send(2);
        ev.update();
        assert_eq!(ev.read(&mut late).collect::<Vec<_>>(), vec![2]);
    }
}
