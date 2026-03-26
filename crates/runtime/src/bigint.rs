//! [`JBigInteger`] — Rust representation of `java.math.BigInteger`.
//!
//! Mapping: `java.math.BigInteger` → `JBigInteger` (backed by `i128`).
//! Handles integers within the `i128` range (~1.7 × 10^38).
//! For larger values, consider using the `num-bigint` crate.

use crate::string::JString;

/// Java `java.math.BigInteger` — arbitrary-precision integer (backed by `i128`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct JBigInteger {
    inner: i128,
}

impl JBigInteger {
    /// Java `BigInteger.valueOf(long)`.
    pub fn from_long(n: i64) -> Self {
        Self { inner: n as i128 }
    }

    /// Java `new BigInteger(String)`.
    pub fn from_string(s: JString) -> Self {
        let v: i128 = s.as_str().trim().parse().unwrap_or(0);
        Self { inner: v }
    }

    /// Java `a.add(b)`.
    pub fn add(&self, other: JBigInteger) -> JBigInteger {
        JBigInteger {
            inner: self.inner.wrapping_add(other.inner),
        }
    }

    /// Java `a.subtract(b)`.
    pub fn subtract(&self, other: JBigInteger) -> JBigInteger {
        JBigInteger {
            inner: self.inner.wrapping_sub(other.inner),
        }
    }

    /// Java `a.multiply(b)`.
    pub fn multiply(&self, other: JBigInteger) -> JBigInteger {
        JBigInteger {
            inner: self.inner.wrapping_mul(other.inner),
        }
    }

    /// Java `a.divide(b)`.
    pub fn divide(&self, other: JBigInteger) -> JBigInteger {
        JBigInteger {
            inner: self.inner / other.inner,
        }
    }

    /// Java `a.mod(b)`.
    #[allow(non_snake_case)]
    pub fn mod_(&self, other: JBigInteger) -> JBigInteger {
        JBigInteger {
            inner: self.inner.rem_euclid(other.inner),
        }
    }

    /// Java `a.remainder(b)`.
    pub fn remainder(&self, other: JBigInteger) -> JBigInteger {
        JBigInteger {
            inner: self.inner % other.inner,
        }
    }

    /// Java `a.pow(exponent)`.
    pub fn pow(&self, exp: i32) -> JBigInteger {
        JBigInteger {
            inner: self.inner.wrapping_pow(exp as u32),
        }
    }

    /// Java `a.abs()`.
    pub fn abs(&self) -> JBigInteger {
        JBigInteger {
            inner: self.inner.abs(),
        }
    }

    /// Java `a.negate()`.
    pub fn negate(&self) -> JBigInteger {
        JBigInteger {
            inner: -self.inner,
        }
    }

    /// Java `a.gcd(b)`.
    pub fn gcd(&self, other: JBigInteger) -> JBigInteger {
        fn gcd_impl(a: i128, b: i128) -> i128 {
            if b == 0 {
                a.abs()
            } else {
                gcd_impl(b, a % b)
            }
        }
        JBigInteger {
            inner: gcd_impl(self.inner, other.inner),
        }
    }

    /// Java `a.compareTo(b)`.
    pub fn compareTo(&self, other: JBigInteger) -> i32 {
        match self.inner.cmp(&other.inner) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    /// Java `a.intValue()`.
    pub fn intValue(&self) -> i32 {
        self.inner as i32
    }

    /// Java `a.longValue()`.
    pub fn longValue(&self) -> i64 {
        self.inner as i64
    }

    /// Java `a.doubleValue()`.
    pub fn doubleValue(&self) -> f64 {
        self.inner as f64
    }

    /// Java `a.toString()`.
    pub fn toString(&self) -> JString {
        JString::from(self.inner.to_string().as_str())
    }

    /// Java `a.bitLength()`.
    pub fn bitLength(&self) -> i32 {
        if self.inner == 0 {
            0
        } else {
            (128 - self.inner.abs().leading_zeros()) as i32
        }
    }
}

impl std::fmt::Display for JBigInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}
