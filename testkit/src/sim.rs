//! Deterministic simulation: virtual time and a faulty network, both driven by
//! one seed, in the style of `TigerBeetle`'s simulator.
//!
//! The code under test must not do its own I/O or read the clock: it takes
//! messages and the current time as inputs and returns what it wants to send.
//! The test owns the loop: tick the clock, deliver what is due, feed each
//! message and the time to its node, send what the node returns, check
//! invariants, and stop at a tick limit. Then [`Network::heal`] the faults
//! and check the system recovers. `tests/simulation.rs` is a worked example.

use std::collections::BTreeMap;
use std::fmt::{self, Display};

use crate::check::{Failure, fail};
use crate::rng::Rng;

/// How hostile the network is. [`Faults::swarm`] picks a different mix per
/// seed, so some runs are calm and some are brutal, which finds more bugs than
/// one fixed middle setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Faults {
    /// Chance in 1000 that a message is lost.
    pub drop_per_mille: u64,
    /// Chance in 1000 that a message is delivered twice.
    pub duplicate_per_mille: u64,
    /// Each delivery takes `1..=1 + max_delay` ticks, so messages reorder.
    pub max_delay: u64,
}

impl Faults {
    /// A perfect network: in order, one tick per message.
    pub const NONE: Self = Self {
        drop_per_mille: 0,
        duplicate_per_mille: 0,
        max_delay: 0,
    };

    /// A random mix of faults for this run.
    pub fn swarm(rng: &mut Rng) -> Self {
        let mut choose = |options: &[u64]| rng.pick(options).copied().unwrap_or_default();
        Self {
            drop_per_mille: choose(&[0, 0, 10, 100, 300]),
            duplicate_per_mille: choose(&[0, 0, 10, 100]),
            max_delay: choose(&[0, 3, 50]),
        }
    }
}

impl Display for Faults {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "drop {}/1000, duplicate {}/1000, delay up to {} ticks",
            self.drop_per_mille, self.duplicate_per_mille, self.max_delay
        )
    }
}

/// A message in flight or delivered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope<M> {
    /// The virtual time it arrives.
    pub at: u64,
    /// The sending node.
    pub from: usize,
    /// The receiving node.
    pub to: usize,
    /// The payload.
    pub message: M,
}

/// A network that loses, duplicates, delays and reorders messages.
#[derive(Debug)]
pub struct Network<M> {
    rng: Rng,
    faults: Faults,
    now: u64,
    sequence: u64,
    queue: BTreeMap<(u64, u64), Envelope<M>>,
}

impl<M: Clone> Network<M> {
    /// An empty network at time 0.
    #[must_use]
    pub const fn new(rng: Rng, faults: Faults) -> Self {
        Self {
            rng,
            faults,
            now: 0,
            sequence: 0,
            queue: BTreeMap::new(),
        }
    }

    /// The current virtual time.
    #[must_use]
    pub const fn now(&self) -> u64 {
        self.now
    }

    /// The faults applied to messages sent from now on.
    #[must_use]
    pub const fn faults(&self) -> Faults {
        self.faults
    }

    /// Messages sent but not yet delivered.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.queue.len()
    }

    /// Stop injecting faults into new messages; ones in flight still arrive.
    pub const fn heal(&mut self) {
        self.faults = Faults::NONE;
    }

    /// Send `message`, subject to the current faults.
    ///
    /// # Errors
    /// Fails only if virtual time would overflow.
    pub fn send(&mut self, from: usize, to: usize, message: M) -> Result<(), Failure> {
        if self.rng.chance(self.faults.drop_per_mille, 1000) {
            return Ok(());
        }
        if self.rng.chance(self.faults.duplicate_per_mille, 1000) {
            self.enqueue(from, to, message.clone())?;
        }
        self.enqueue(from, to, message)
    }

    fn enqueue(&mut self, from: usize, to: usize, message: M) -> Result<(), Failure> {
        let span = self
            .faults
            .max_delay
            .checked_add(1)
            .ok_or_else(|| fail("delay overflow"))?;
        let delay = self.rng.below(span).unwrap_or_default();
        let at = self
            .now
            .checked_add(1)
            .and_then(|next| next.checked_add(delay))
            .ok_or_else(|| fail("virtual time overflow"))?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| fail("sequence overflow"))?;
        self.queue.insert(
            (at, self.sequence),
            Envelope {
                at,
                from,
                to,
                message,
            },
        );
        Ok(())
    }

    /// Advance virtual time by one tick.
    ///
    /// # Errors
    /// Fails only if virtual time would overflow.
    pub fn tick(&mut self) -> Result<u64, Failure> {
        self.now = self
            .now
            .checked_add(1)
            .ok_or_else(|| fail("virtual time overflow"))?;
        Ok(self.now)
    }

    /// The earliest message due by now, if any. Call until `None`, then tick.
    pub fn deliver(&mut self) -> Option<Envelope<M>> {
        let entry = self.queue.first_entry()?;
        if entry.get().at > self.now {
            return None;
        }
        Some(entry.remove())
    }
}

#[cfg(test)]
mod tests {
    use super::{Faults, Network};
    use crate::check::{Failure, ensure, ensure_eq};
    use crate::rng::Rng;

    /// Everything delivered, as `(arrival, message)`, until the network is empty.
    fn drain(network: &mut Network<u32>) -> Result<Vec<(u64, u32)>, Failure> {
        let mut delivered = Vec::new();
        while network.in_flight() > 0 {
            network.tick()?;
            delivered.extend(std::iter::from_fn(|| network.deliver()).map(|e| (e.at, e.message)));
        }
        Ok(delivered)
    }

    #[test]
    fn a_perfect_network_delivers_in_order() -> Result<(), Failure> {
        let mut network = Network::new(Rng::new(1), Faults::NONE);
        for message in 0..3 {
            network.send(0, 1, message)?;
        }
        ensure_eq(
            &drain(&mut network)?,
            &vec![(1, 0), (1, 1), (1, 2)],
            "arrivals",
        )
    }

    #[test]
    fn the_same_seed_gives_the_same_history() -> Result<(), Failure> {
        let history = |seed| -> Result<Vec<(u64, u32)>, Failure> {
            let faults = Faults {
                drop_per_mille: 200,
                duplicate_per_mille: 200,
                max_delay: 9,
            };
            let mut network = Network::new(Rng::new(seed), faults);
            for message in 0..50 {
                network.send(0, 1, message)?;
            }
            drain(&mut network)
        };
        let first = history(7)?;
        ensure_eq(&history(7)?, &first, "history for one seed")?;
        ensure(
            history(8)? != first,
            "different seeds gave identical histories",
        )
    }

    #[test]
    fn messages_wait_for_their_tick() -> Result<(), Failure> {
        let mut network = Network::new(Rng::new(1), Faults::NONE);
        network.send(0, 1, 9)?;
        ensure(
            network.deliver().is_none(),
            "delivered before its arrival time",
        )?;
        network.tick()?;
        ensure_eq(
            &network.deliver().map(|e| e.message),
            &Some(9),
            "delivery at its tick",
        )
    }

    #[test]
    fn heal_stops_new_faults() -> Result<(), Failure> {
        let total_loss = Faults {
            drop_per_mille: 1000,
            duplicate_per_mille: 0,
            max_delay: 0,
        };
        let mut network = Network::new(Rng::new(1), total_loss);
        network.send(0, 1, 1)?;
        ensure_eq(&network.in_flight(), &0, "messages surviving total loss")?;
        network.heal();
        network.send(0, 1, 2)?;
        ensure_eq(
            &drain(&mut network)?,
            &vec![(1, 2)],
            "arrivals after healing",
        )?;
        ensure_eq(
            &total_loss.to_string().as_str(),
            &"drop 1000/1000, duplicate 0/1000, delay up to 0 ticks",
            "description",
        )
    }
}
