//! Template: deterministic simulation with swarm faults, after TigerBeetle.
//!
//! The protocol here is a toy (stop-and-wait retransmission with cumulative
//! acks) written in the "sans-I/O" style real code should use: nodes never
//! touch sockets or clocks; they take a message and the time and return what
//! to send. That is what lets one seed drive the whole world.
//!
//! Each case: pick faults for this seed, run with them for a while checking
//! safety after every step, heal the network, then require progress
//! (liveness) within a bound. A failure names the seed, the faults and the
//! tick, and `TEST_SEED=… cargo test --test simulation` replays it exactly.

use testkit::sim::{Envelope, Faults, Network};
use testkit::{Explore, Failure, Rng, ensure_eq, fail};

const SENDER: usize = 0;
const RECEIVER: usize = 1;
const RESEND_AFTER: u64 = 8;
const FAULTY_TICKS: u64 = 2_000;
const HEALED_TICKS: u64 = 2_000;

#[derive(Clone, Debug)]
enum Message {
    Data { seq: usize, value: u32 },
    Ack { next: usize },
}

/// Sends `values` one at a time until each is acknowledged.
struct Sender {
    values: Vec<u32>,
    next_unacked: usize,
    last_sent: Option<u64>,
}

impl Sender {
    fn on_tick(&mut self, now: u64) -> Option<Message> {
        let value = *self.values.get(self.next_unacked)?;
        let due = self
            .last_sent
            .is_none_or(|sent| now.checked_sub(sent).is_some_and(|age| age >= RESEND_AFTER));
        if !due {
            return None;
        }
        self.last_sent = Some(now);
        Some(Message::Data {
            seq: self.next_unacked,
            value,
        })
    }

    fn on_message(&mut self, message: &Message) {
        if let Message::Ack { next } = *message
            && next > self.next_unacked
        {
            self.next_unacked = next;
            self.last_sent = None;
        }
    }
}

/// Accepts values in order, exactly once, and acknowledges cumulatively.
#[derive(Default)]
struct Receiver {
    log: Vec<u32>,
}

impl Receiver {
    fn on_message(&mut self, message: &Message) -> Option<Message> {
        let Message::Data { seq, value } = *message else {
            return None;
        };
        if seq == self.log.len() {
            self.log.push(value);
        }
        Some(Message::Ack {
            next: self.log.len(),
        })
    }
}

/// Run one world from `seed`.
fn simulate(seed: u64) -> Result<(), Failure> {
    let mut rng = Rng::new(seed);
    let faults = Faults::swarm(&mut rng);
    let count = rng.index(40).unwrap_or_default();
    let values: Vec<u32> = rng.bytes(count).into_iter().map(u32::from).collect();
    let mut network = Network::new(rng.fork(), faults);
    let mut sender = Sender {
        values: values.clone(),
        next_unacked: 0,
        last_sent: None,
    };
    let mut receiver = Receiver::default();
    let context = |now: u64| format!("seed {seed:#x}, faults [{faults}], tick {now}");

    for phase_end in [
        FAULTY_TICKS,
        FAULTY_TICKS
            .checked_add(HEALED_TICKS)
            .ok_or_else(|| fail("tick overflow"))?,
    ] {
        while network.now() < phase_end {
            let now = network.tick()?;
            if let Some(message) = sender.on_tick(now) {
                network.send(SENDER, RECEIVER, message)?;
            }
            while let Some(Envelope { to, message, .. }) = network.deliver() {
                if to == RECEIVER {
                    if let Some(reply) = receiver.on_message(&message) {
                        network.send(RECEIVER, SENDER, reply)?;
                    }
                } else {
                    sender.on_message(&message);
                }
            }
            // Safety, every step: the receiver's log is a prefix of what was sent.
            if !values.starts_with(&receiver.log) {
                return Err(fail(format!(
                    "receiver log {:?} is not a prefix of {values:?} ({})",
                    receiver.log,
                    context(now)
                )));
            }
            if receiver.log.len() == values.len() && network.faults() == Faults::NONE {
                break;
            }
        }
        network.heal();
    }
    // Liveness: once healed, everything arrives within the bound.
    ensure_eq(&receiver.log, &values, "delivered values")
        .map_err(|error| error.context(context(network.now())))
}

#[test]
fn delivers_exactly_once_in_order_under_faults() -> Result<(), Failure> {
    Explore::from_env(64)?.check(Rng::next_u64, |seed| simulate(*seed))
}
