//! `JRandom` — Rust implementation of `java.util.Random` and
//! `java.util.concurrent.ThreadLocalRandom`.
//!
//! Uses Java's exact 48-bit multiplicative congruential generator so that
//! seeded instances produce the same sequence as `java.util.Random`.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::time::SystemTime;

use crate::stream::JStream;

/// Multiplier and addend from Java's 48-bit LCG spec.
const MULTIPLIER: u64 = 0x5DEECE66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1u64 << 48) - 1;

/// `java.util.Random` — a seedable pseudo-random number generator using
/// Java's 48-bit LCG algorithm.
#[derive(Clone, Debug)]
pub struct JRandom {
    seed: Cell<u64>,
}

impl JRandom {
    fn initial_seed(seed: i64) -> u64 {
        ((seed as u64) ^ MULTIPLIER) & MASK
    }

    /// Advance the LCG by one step and return `bits` upper bits as an `i32`.
    fn next_bits(&self, bits: u32) -> i32 {
        let mut s = self.seed.get();
        s = s.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        self.seed.set(s);
        (s >> (48 - bits)) as i32
    }

    /// `new Random()` — seeded from the current wall clock.
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as i64 ^ (d.as_secs() as i64))
            .unwrap_or(12345);
        Self {
            seed: Cell::new(Self::initial_seed(seed)),
        }
    }

    /// `new Random(seed)` — reproducible sequence.
    pub fn new_seed(seed: i64) -> Self {
        Self {
            seed: Cell::new(Self::initial_seed(seed)),
        }
    }

    /// `ThreadLocalRandom.current()` — returns a thread-local `JRandom`.
    /// Each call fetches the same per-thread instance (state is preserved
    /// across calls on the same thread).
    pub fn thread_local_current() -> JRandom {
        THREAD_LOCAL_RNG.with(|r| r.clone())
    }

    /// `Math.random()` — returns a pseudo-random `f64` in `[0.0, 1.0)`.
    pub fn math_random() -> f64 {
        THREAD_LOCAL_RNG.with(|r| r.nextDouble())
    }

    // ── Java API ────────────────────────────────────────────────────────────

    /// `nextInt()` — any 32-bit signed integer.
    pub fn nextInt(&self) -> i32 {
        self.next_bits(32)
    }

    /// `nextInt(bound)` — uniformly distributed in `[0, bound)`.
    pub fn nextInt_bound(&self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        // Power-of-two fast path (matches Java spec).
        if bound & (bound - 1) == 0 {
            return (((bound as i64) * (self.next_bits(31) as i64)) >> 31) as i32;
        }
        loop {
            let bits = self.next_bits(31);
            let val = bits % bound;
            if bits - val + (bound - 1) >= 0 {
                return val;
            }
        }
    }

    /// `nextLong()` — any 64-bit signed integer.
    pub fn nextLong(&self) -> i64 {
        ((self.next_bits(32) as i64) << 32) + (self.next_bits(32) as i64)
    }

    /// `nextDouble()` — uniformly distributed in `[0.0, 1.0)`.
    pub fn nextDouble(&self) -> f64 {
        let hi = (self.next_bits(26) as i64) << 27;
        let lo = self.next_bits(27) as i64;
        (hi + lo) as f64 / (1u64 << 53) as f64
    }

    /// `nextBoolean()`.
    pub fn nextBoolean(&self) -> bool {
        self.next_bits(1) != 0
    }

    /// `nextGaussian()` — Box-Muller transform, mean 0 variance 1.
    pub fn nextGaussian(&self) -> f64 {
        let u1 = self.nextDouble();
        let u2 = self.nextDouble();
        // Guard against log(0) in the rare case nextDouble returns 0.0.
        let u1 = if u1 == 0.0 { f64::MIN_POSITIVE } else { u1 };
        (-2.0_f64 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// `ints(n, origin, bound)` — returns a `JStream<i32>` of `n` values in
    /// `[origin, bound)`.
    pub fn ints_stream(&self, n: i64, origin: i32, bound: i32) -> JStream<i32> {
        let range = bound - origin;
        let v: Vec<i32> = (0..n).map(|_| origin + self.nextInt_bound(range)).collect();
        JStream::new(v)
    }
}

impl Default for JRandom {
    fn default() -> Self {
        Self::new()
    }
}

// ── Thread-local instance ────────────────────────────────────────────────────

thread_local! {
    static THREAD_LOCAL_RNG: JRandom = JRandom::new();
}
