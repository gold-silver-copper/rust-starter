# Deterministic simulation

For anything with time, concurrency, retries or a network, run the whole
system inside one process on a virtual clock, with every random choice taken
from one seed. A failure then replays exactly from that seed, on any machine.
This is TigerBeetle's simulator (the VOPR) reduced to its core; ck's fuzzer
and koh's seeded fake network are smaller versions of the same idea.

`tests/simulation.rs` is the worked example.

## Shape the code for it

The code under test must not perform I/O or read the clock. Each node is a
state machine: it takes a message (or a tick) and the current time, and
returns the messages it wants to send. This is often called "sans-I/O".
Production wraps it in a thin layer that does the real sockets and timers;
tests wrap it in `testkit::sim::Network`.

koh's `ClientSession` and `ServerConn` work this way: they take the time as
an argument and never sleep or call `Instant::now()`.

## The loop

```text
pick faults for this seed (swarm)
for tick in 0..LIMIT:
    advance the clock; let each node act on the time
    deliver every message due now; send what the receivers return
    check safety invariants            -> fail with seed, faults and tick
heal the network
run up to a second limit
check liveness: the system finished  -> fail with seed, faults and tick
```

- **Safety** is checked after every step ("the log is a prefix of what was
  sent", "no two leaders in one term"). Check both what must happen and what
  must not.
- **Liveness** is checked after healing: with faults gone, the system must make
  progress within a bound. A system that is merely safe can be stuck forever.
- **Bounds** everywhere. Every loop has a tick limit; exceeding it is a
  failure that says which limit, never a hang.

## Swarm testing

`Faults::swarm(&mut rng)` picks a different mix per seed: some worlds have no
loss and long delays, some lose 30% of messages, some duplicate. One fixed
"medium" setting misses bugs that need an extreme in one dimension and calm in
another. Print the chosen faults in every failure (`Faults` implements
`Display`).

## Seeds in CI

`Explore::from_env(64)?.check(Rng::next_u64, |seed| simulate(*seed))` runs 64
worlds. Locally the seed is fixed. In CI it comes from the commit, so each
push explores new worlds, and the scheduled job runs many more with
`TEST_CASES`. A failure prints the world's seed, its faults and the tick, and
`TEST_SEED=… cargo test --test simulation` replays it.

## Check the simulator can fail

A simulation that never fails may not be checking anything. When adding one,
inject a bug the invariants should catch (for the example: let the receiver
accept a duplicate) and confirm the test fails with a useful report. Then
remove the bug. `cargo mutants` automates this on the real code.

## Beyond the example

- Add **storage faults** the same way: a fake disk that loses unsynced
  writes, corrupts a sector or returns errors, driven by the same `Rng`.
- Add **process crashes and restarts**: drop a node's volatile state at a
  random tick and rebuild it from its storage.
- **Record histories** (every request and its response) and check them
  against a simple model afterwards, as Jepsen does for linearizability.
- For async code on tokio, `turmoil` runs real tokio code on a simulated
  network and returns `Result`. `loom` and `shuttle` explore thread
  interleavings but report failures by panicking, so they need their own
  crate outside the strict lint policy.
