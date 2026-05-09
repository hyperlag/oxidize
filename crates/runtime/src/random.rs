//! `JRandom` — Rust implementation of `java.util.Random` and
//! `java.util.concurrent.ThreadLocalRandom`.
//!
//! Uses Java's exact 48-bit multiplicative congruential generator so that
//! seeded instances produce the same sequence as `java.util.Random`.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;
use std::time::SystemTime;

use crate::stream::JStream;

/// Multiplier and addend from Java's 48-bit LCG spec.
const MULTIPLIER: u64 = 0x5DEECE66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1u64 << 48) - 1;

/// `java.util.Random` — a seedable pseudo-random number generator using
/// Java's 48-bit LCG algorithm.
///
/// The seed is stored behind an `Rc<Cell<u64>>` so that clones (e.g. from
/// `ThreadLocalRandom.current()`) share the same underlying state rather than
/// forking an independent sequence.
#[derive(Clone, Debug)]
pub struct JRandom {
    seed: Rc<Cell<u64>>,
    /// Cached second Gaussian value (Java `haveNextNextGaussian`).
    have_next_gaussian: Cell<bool>,
    /// Cached second Gaussian value (Java `nextNextGaussian`).
    next_next_gaussian: Cell<f64>,
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
            seed: Rc::new(Cell::new(Self::initial_seed(seed))),
            have_next_gaussian: Cell::new(false),
            next_next_gaussian: Cell::new(0.0),
        }
    }

    /// `new Random(seed)` — reproducible sequence.
    pub fn new_seed(seed: i64) -> Self {
        Self {
            seed: Rc::new(Cell::new(Self::initial_seed(seed))),
            have_next_gaussian: Cell::new(false),
            next_next_gaussian: Cell::new(0.0),
        }
    }

    /// `ThreadLocalRandom.current()` — returns a handle to the thread-local
    /// `JRandom`. The returned value shares the underlying seed state with the
    /// thread-local instance (via `Rc`), so subsequent calls on the same
    /// thread continue the same sequence.
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
        if bound <= 0 {
            panic!("IllegalArgumentException: bound must be positive");
        }
        // Power-of-two fast path (matches Java spec).
        if bound & (bound - 1) == 0 {
            return (((bound as i64) * (self.next_bits(31) as i64)) >> 31) as i32;
        }
        loop {
            let bits = self.next_bits(31);
            let val = bits % bound;
            // Use i64 arithmetic to avoid i32 overflow in debug builds —
            // Java relies on 32-bit wraparound, but Rust panics on overflow.
            if (bits as i64) - (val as i64) + ((bound - 1) as i64) >= 0 {
                return val;
            }
        }
    }

    /// `nextInt(origin, bound)` — uniformly distributed in `[origin, bound)`.
    pub fn nextInt_origin_bound(&self, origin: i32, bound: i32) -> i32 {
        if origin >= bound {
            panic!("IllegalArgumentException: bound must be greater than origin");
        }
        let n = bound.wrapping_sub(origin);
        if n > 0 {
            self.nextInt_bound(n) + origin
        } else {
            // Range is not representable as a positive i32; use rejection
            // sampling (matches Java's RandomGenerator.nextInt(origin, bound)).
            loop {
                let r = self.nextInt();
                if r >= origin && r < bound {
                    return r;
                }
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

    /// `nextGaussian()` — mean 0, variance 1.
    ///
    /// Mirrors Java's `java.util.Random.nextGaussian()` exactly: uses the
    /// Marsaglia polar method and caches the second value so that consecutive
    /// calls consume the same number of `nextDouble()` invocations as Java.
    pub fn nextGaussian(&self) -> f64 {
        if self.have_next_gaussian.get() {
            self.have_next_gaussian.set(false);
            return self.next_next_gaussian.get();
        }
        loop {
            let v1 = 2.0 * self.nextDouble() - 1.0;
            let v2 = 2.0 * self.nextDouble() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                let multiplier = (-2.0 * s.ln() / s).sqrt();
                self.next_next_gaussian.set(v2 * multiplier);
                self.have_next_gaussian.set(true);
                return v1 * multiplier;
            }
        }
    }

    /// `ints(n, origin, bound)` — returns a `JStream<i32>` of `n` values in
    /// `[origin, bound)`.
    pub fn ints(&self, n: i64, origin: i32, bound: i32) -> JStream<i32> {
        if n < 0 {
            panic!("IllegalArgumentException: size must be non-negative");
        }
        if origin >= bound {
            panic!("IllegalArgumentException: bound must be greater than origin");
        }
        let v: Vec<i32> = (0..n)
            .map(|_| self.nextInt_origin_bound(origin, bound))
            .collect();
        JStream::new(v)
    }

    /// Alias for [`ints`](Self::ints) — kept for backwards compatibility.
    pub fn ints_stream(&self, n: i64, origin: i32, bound: i32) -> JStream<i32> {
        self.ints(n, origin, bound)
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
