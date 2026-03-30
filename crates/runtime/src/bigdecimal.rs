#![allow(non_snake_case)]
//! [`JBigDecimal`] — Rust representation of `java.math.BigDecimal`.
//!
//! Backed by an `(i128, i32)` pair representing `unscaledValue × 10^(−scale)`,
//! mirroring Java's internal representation.  Supports the most commonly used
//! `BigDecimal` operations for transpiled code.

use crate::JString;
use std::fmt;

/// Rounding modes corresponding to `java.math.RoundingMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JRoundingMode {
    Up,
    Down,
    Ceiling,
    Floor,
    #[default]
    HalfUp,
    HalfDown,
    HalfEven,
    Unnecessary,
}

/// Precision + rounding context, corresponding to `java.math.MathContext`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JMathContext {
    pub precision: i32,
    pub rounding_mode: JRoundingMode,
}

impl JMathContext {
    pub fn new(precision: i32, rounding_mode: JRoundingMode) -> Self {
        JMathContext {
            precision,
            rounding_mode,
        }
    }

    pub fn getPrecision(&self) -> i32 {
        self.precision
    }

    pub fn getRoundingMode(&self) -> JRoundingMode {
        self.rounding_mode
    }
}

/// Named MathContext constants.
impl JMathContext {
    pub fn decimal32() -> Self {
        JMathContext::new(7, JRoundingMode::HalfEven)
    }
    pub fn decimal64() -> Self {
        JMathContext::new(16, JRoundingMode::HalfEven)
    }
    pub fn decimal128() -> Self {
        JMathContext::new(34, JRoundingMode::HalfEven)
    }
    pub fn unlimited() -> Self {
        JMathContext::new(0, JRoundingMode::HalfUp)
    }
}

/// Rust equivalent of `java.math.BigDecimal`.
///
/// Internally stores `(unscaled: i128, scale: i32)` so the decimal value is
/// `unscaled × 10^(−scale)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JBigDecimal {
    unscaled: i128,
    scale: i32,
}

// ─── Constructors ────────────────────────────────────────────────────────────

impl JBigDecimal {
    /// `new BigDecimal(String)`
    pub fn from_string(s: JString) -> Self {
        Self::parse(s.as_str())
    }

    /// `new BigDecimal(int)` / `new BigDecimal(long)`
    pub fn from_long(n: i64) -> Self {
        JBigDecimal {
            unscaled: n as i128,
            scale: 0,
        }
    }

    /// `new BigDecimal(double)`
    pub fn from_double(d: f64) -> Self {
        Self::parse(&format!("{d}"))
    }

    /// `BigDecimal.valueOf(long)` / `BigDecimal.valueOf(long, int)`
    pub fn value_of(val: i64) -> Self {
        Self::from_long(val)
    }

    /// `BigDecimal.valueOf(double)`
    pub fn value_of_double(val: f64) -> Self {
        Self::parse(&format!("{val}"))
    }

    /// `BigDecimal.valueOf(long, scale)`
    pub fn value_of_scaled(unscaled: i64, scale: i32) -> Self {
        JBigDecimal {
            unscaled: unscaled as i128,
            scale,
        }
    }

    /// Named constants.
    pub fn zero() -> Self {
        JBigDecimal {
            unscaled: 0,
            scale: 0,
        }
    }
    pub fn one() -> Self {
        JBigDecimal {
            unscaled: 1,
            scale: 0,
        }
    }
    pub fn ten() -> Self {
        JBigDecimal {
            unscaled: 10,
            scale: 0,
        }
    }

    fn parse(s: &str) -> Self {
        let s = s.trim();
        if let Some(dot_pos) = s.find('.') {
            let int_part = &s[..dot_pos];
            let frac_part = &s[dot_pos + 1..];
            let scale = frac_part.len() as i32;
            let combined = format!("{int_part}{frac_part}");
            let unscaled: i128 = combined.parse().unwrap_or(0);
            JBigDecimal { unscaled, scale }
        } else if let Some(e_pos) = s.to_ascii_lowercase().find('e') {
            let mantissa: i128 = s[..e_pos].parse().unwrap_or(0);
            let exp: i32 = s[e_pos + 1..].parse().unwrap_or(0);
            JBigDecimal {
                unscaled: mantissa,
                scale: -exp,
            }
        } else {
            let unscaled: i128 = s.parse().unwrap_or(0);
            JBigDecimal { unscaled, scale: 0 }
        }
    }
}

// ─── Arithmetic ──────────────────────────────────────────────────────────────

impl JBigDecimal {
    fn align(a: &JBigDecimal, b: &JBigDecimal) -> (i128, i128, i32) {
        if a.scale == b.scale {
            (a.unscaled, b.unscaled, a.scale)
        } else if a.scale > b.scale {
            let diff = a.scale - b.scale;
            (a.unscaled, b.unscaled * pow10(diff), a.scale)
        } else {
            let diff = b.scale - a.scale;
            (a.unscaled * pow10(diff), b.unscaled, b.scale)
        }
    }

    /// Java `BigDecimal.add(BigDecimal)`
    pub fn add(&self, other: JBigDecimal) -> JBigDecimal {
        let (a, b, s) = Self::align(self, &other);
        JBigDecimal {
            unscaled: a + b,
            scale: s,
        }
    }

    /// Java `BigDecimal.subtract(BigDecimal)`
    pub fn subtract(&self, other: JBigDecimal) -> JBigDecimal {
        let (a, b, s) = Self::align(self, &other);
        JBigDecimal {
            unscaled: a - b,
            scale: s,
        }
    }

    /// Java `BigDecimal.multiply(BigDecimal)`
    pub fn multiply(&self, other: JBigDecimal) -> JBigDecimal {
        JBigDecimal {
            unscaled: self.unscaled * other.unscaled,
            scale: self.scale + other.scale,
        }
    }

    /// Java `BigDecimal.divide(BigDecimal)`
    ///
    /// Uses a default scale of `max(this.scale, dividend.scale) + 10` with
    /// half-up rounding when the result is not exact.
    pub fn divide(&self, other: JBigDecimal) -> JBigDecimal {
        self.divide_with_scale(
            other,
            std::cmp::max(self.scale, other.scale) + 10,
            JRoundingMode::HalfUp,
        )
    }

    /// Java `BigDecimal.divide(BigDecimal, scale, RoundingMode)`
    pub fn divide_with_scale(
        &self,
        other: JBigDecimal,
        new_scale: i32,
        rounding: JRoundingMode,
    ) -> JBigDecimal {
        if other.unscaled == 0 {
            panic!("Division by zero");
        }
        // We want result.unscaled / 10^new_scale = self / other
        // result.unscaled = self.unscaled * 10^(new_scale - self.scale + other.scale) / other.unscaled
        let shift = new_scale - self.scale + other.scale;
        let numerator = if shift >= 0 {
            self.unscaled * pow10(shift)
        } else {
            self.unscaled / pow10(-shift)
        };
        let (quotient, remainder) = (numerator / other.unscaled, numerator % other.unscaled);
        let unscaled = round_div(quotient, remainder, other.unscaled, rounding);
        JBigDecimal {
            unscaled,
            scale: new_scale,
        }
    }

    /// Java `BigDecimal.remainder(BigDecimal)`
    pub fn remainder(&self, other: JBigDecimal) -> JBigDecimal {
        let (a, b, s) = Self::align(self, &other);
        JBigDecimal {
            unscaled: a % b,
            scale: s,
        }
    }

    /// Java `BigDecimal.abs()`
    pub fn abs(&self) -> JBigDecimal {
        JBigDecimal {
            unscaled: self.unscaled.abs(),
            scale: self.scale,
        }
    }

    /// Java `BigDecimal.negate()`
    pub fn negate(&self) -> JBigDecimal {
        JBigDecimal {
            unscaled: -self.unscaled,
            scale: self.scale,
        }
    }

    /// Java `BigDecimal.pow(int)`
    pub fn pow(&self, exp: i32) -> JBigDecimal {
        let mut result = JBigDecimal::one();
        for _ in 0..exp {
            result = result.multiply(*self);
        }
        result
    }

    /// Java `BigDecimal.max(BigDecimal)`
    pub fn max(&self, other: JBigDecimal) -> JBigDecimal {
        if self.compareTo(other) >= 0 {
            *self
        } else {
            other
        }
    }

    /// Java `BigDecimal.min(BigDecimal)`
    pub fn min(&self, other: JBigDecimal) -> JBigDecimal {
        if self.compareTo(other) <= 0 {
            *self
        } else {
            other
        }
    }
}

// ─── Comparison ──────────────────────────────────────────────────────────────

impl JBigDecimal {
    /// Java `BigDecimal.compareTo(BigDecimal)`.
    /// Unlike `equals`, this ignores scale: `2.0.compareTo(2.00) == 0`.
    pub fn compareTo(&self, other: JBigDecimal) -> i32 {
        let (a, b, _) = Self::align(self, &other);
        a.cmp(&b) as i32
    }

    /// Java `BigDecimal.signum()`
    pub fn signum(&self) -> i32 {
        self.unscaled.signum() as i32
    }

    /// Java `BigDecimal.equals(BigDecimal)` — compares value AND scale.
    pub fn equals(&self, other: JBigDecimal) -> bool {
        self.unscaled == other.unscaled && self.scale == other.scale
    }
}

impl PartialOrd for JBigDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JBigDecimal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let (a, b, _) = Self::align(self, other);
        a.cmp(&b)
    }
}

impl std::hash::Hash for JBigDecimal {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Normalize so that 2.0 and 2.00 hash the same
        let norm = self.stripTrailingZeros();
        norm.unscaled.hash(state);
        norm.scale.hash(state);
    }
}

// ─── Scale ───────────────────────────────────────────────────────────────────

impl JBigDecimal {
    /// Java `BigDecimal.scale()`
    pub fn scale(&self) -> i32 {
        self.scale
    }

    /// Java `BigDecimal.unscaledValue()` → i128 (not JBigInteger for simplicity)
    pub fn unscaledValue(&self) -> i128 {
        self.unscaled
    }

    /// Java `BigDecimal.precision()`
    pub fn precision(&self) -> i32 {
        if self.unscaled == 0 {
            return 1;
        }
        let s = self.unscaled.abs().to_string();
        s.len() as i32
    }

    /// Java `BigDecimal.setScale(int, RoundingMode)`
    #[allow(non_snake_case)]
    pub fn setScale(&self, new_scale: i32, rounding: JRoundingMode) -> JBigDecimal {
        if new_scale == self.scale {
            return *self;
        }
        if new_scale > self.scale {
            let diff = new_scale - self.scale;
            JBigDecimal {
                unscaled: self.unscaled * pow10(diff),
                scale: new_scale,
            }
        } else {
            let diff = self.scale - new_scale;
            let divisor = pow10(diff);
            let (quotient, remainder) = (self.unscaled / divisor, self.unscaled % divisor);
            JBigDecimal {
                unscaled: round_div(quotient, remainder, divisor, rounding),
                scale: new_scale,
            }
        }
    }

    /// Java `BigDecimal.stripTrailingZeros()`
    #[allow(non_snake_case)]
    pub fn stripTrailingZeros(&self) -> JBigDecimal {
        if self.unscaled == 0 {
            return JBigDecimal {
                unscaled: 0,
                scale: 0,
            };
        }
        let mut u = self.unscaled;
        let mut s = self.scale;
        while u % 10 == 0 {
            u /= 10;
            s -= 1;
        }
        JBigDecimal {
            unscaled: u,
            scale: s,
        }
    }

    /// Java `BigDecimal.movePointLeft(int)`
    #[allow(non_snake_case)]
    pub fn movePointLeft(&self, n: i32) -> JBigDecimal {
        JBigDecimal {
            unscaled: self.unscaled,
            scale: self.scale + n,
        }
    }

    /// Java `BigDecimal.movePointRight(int)`
    #[allow(non_snake_case)]
    pub fn movePointRight(&self, n: i32) -> JBigDecimal {
        JBigDecimal {
            unscaled: self.unscaled,
            scale: self.scale - n,
        }
    }
}

// ─── Conversion ──────────────────────────────────────────────────────────────

impl JBigDecimal {
    /// Java `BigDecimal.intValue()`
    #[allow(non_snake_case)]
    pub fn intValue(&self) -> i32 {
        self.as_f64() as i32
    }

    /// Java `BigDecimal.longValue()`
    #[allow(non_snake_case)]
    pub fn longValue(&self) -> i64 {
        self.as_f64() as i64
    }

    /// Java `BigDecimal.doubleValue()`
    #[allow(non_snake_case)]
    pub fn doubleValue(&self) -> f64 {
        self.as_f64()
    }

    fn as_f64(&self) -> f64 {
        if self.scale >= 0 {
            self.unscaled as f64 / 10_f64.powi(self.scale)
        } else {
            self.unscaled as f64 * 10_f64.powi(-self.scale)
        }
    }

    /// Java `BigDecimal.toBigInteger()` — returns the integer part.
    #[allow(non_snake_case)]
    pub fn toBigInteger(&self) -> crate::JBigInteger {
        crate::JBigInteger::from_long(self.longValue())
    }

    /// Java `BigDecimal.toString()`
    #[allow(clippy::inherent_to_string)]
    pub fn toString(&self) -> JString {
        JString::from(format!("{self}").as_str())
    }

    /// Java `BigDecimal.toPlainString()` — same as Display for our impl.
    #[allow(non_snake_case)]
    pub fn toPlainString(&self) -> JString {
        self.toString()
    }
}

// ─── Display ─────────────────────────────────────────────────────────────────

impl fmt::Display for JBigDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scale <= 0 {
            // No fractional part — scale up
            let factor = pow10(-self.scale);
            write!(f, "{}", self.unscaled * factor)
        } else {
            let is_negative = self.unscaled < 0;
            let abs_unscaled = self.unscaled.unsigned_abs();
            let s = abs_unscaled.to_string();
            let scale = self.scale as usize;
            if is_negative {
                write!(f, "-")?;
            }
            if s.len() <= scale {
                // e.g. unscaled=5, scale=3 → "0.005"
                write!(f, "0.")?;
                for _ in 0..(scale - s.len()) {
                    write!(f, "0")?;
                }
                write!(f, "{s}")
            } else {
                let (int_part, frac_part) = s.split_at(s.len() - scale);
                write!(f, "{int_part}.{frac_part}")
            }
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn pow10(n: i32) -> i128 {
    if n <= 0 {
        return 1;
    }
    let mut result: i128 = 1;
    for _ in 0..n {
        result *= 10;
    }
    result
}

fn round_div(quotient: i128, remainder: i128, divisor: i128, mode: JRoundingMode) -> i128 {
    if remainder == 0 {
        return quotient;
    }

    let abs_rem = remainder.unsigned_abs();
    let abs_div = divisor.unsigned_abs();
    // Use 2*abs_rem vs abs_div comparison to avoid integer division precision loss
    let doubled = abs_rem * 2;
    let is_negative = (remainder < 0) != (divisor < 0);

    match mode {
        JRoundingMode::Down => quotient,
        JRoundingMode::Up => {
            if is_negative {
                quotient - 1
            } else {
                quotient + 1
            }
        }
        JRoundingMode::Ceiling => {
            if is_negative {
                quotient
            } else {
                quotient + 1
            }
        }
        JRoundingMode::Floor => {
            if is_negative {
                quotient - 1
            } else {
                quotient
            }
        }
        JRoundingMode::HalfUp => {
            if doubled >= abs_div {
                if is_negative {
                    quotient - 1
                } else {
                    quotient + 1
                }
            } else {
                quotient
            }
        }
        JRoundingMode::HalfDown => {
            if doubled > abs_div {
                if is_negative {
                    quotient - 1
                } else {
                    quotient + 1
                }
            } else {
                quotient
            }
        }
        JRoundingMode::HalfEven => {
            if doubled > abs_div || (doubled == abs_div && quotient % 2 != 0) {
                if is_negative {
                    quotient - 1
                } else {
                    quotient + 1
                }
            } else {
                quotient
            }
        }
        JRoundingMode::Unnecessary => {
            if remainder != 0 {
                panic!("Rounding necessary but UNNECESSARY mode specified");
            }
            quotient
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string_basic() {
        let d = JBigDecimal::from_string(JString::from("3.14"));
        assert_eq!(d.unscaled, 314);
        assert_eq!(d.scale, 2);
        assert_eq!(format!("{d}"), "3.14");
    }

    #[test]
    fn from_long() {
        let d = JBigDecimal::from_long(42);
        assert_eq!(format!("{d}"), "42");
    }

    #[test]
    fn add_same_scale() {
        let a = JBigDecimal::from_string(JString::from("1.5"));
        let b = JBigDecimal::from_string(JString::from("2.3"));
        assert_eq!(format!("{}", a.add(b)), "3.8");
    }

    #[test]
    fn add_different_scale() {
        let a = JBigDecimal::from_string(JString::from("1.5"));
        let b = JBigDecimal::from_string(JString::from("2.30"));
        assert_eq!(format!("{}", a.add(b)), "3.80");
    }

    #[test]
    fn multiply() {
        let a = JBigDecimal::from_string(JString::from("2.5"));
        let b = JBigDecimal::from_string(JString::from("4.0"));
        assert_eq!(format!("{}", a.multiply(b)), "10.00");
    }

    #[test]
    fn divide_exact() {
        let a = JBigDecimal::from_string(JString::from("10"));
        let b = JBigDecimal::from_string(JString::from("4"));
        let r = a.divide_with_scale(b, 2, JRoundingMode::HalfUp);
        assert_eq!(format!("{r}"), "2.50");
    }

    #[test]
    fn compare_to() {
        let a = JBigDecimal::from_string(JString::from("2.0"));
        let b = JBigDecimal::from_string(JString::from("2.00"));
        assert_eq!(a.compareTo(b), 0);
    }

    #[test]
    fn set_scale() {
        let d = JBigDecimal::from_string(JString::from("3.14159"));
        let r = d.setScale(2, JRoundingMode::HalfUp);
        assert_eq!(format!("{r}"), "3.14");
    }

    #[test]
    fn strip_trailing_zeros() {
        let d = JBigDecimal::from_string(JString::from("10.00"));
        let r = d.stripTrailingZeros();
        assert_eq!(format!("{r}"), "10");
        assert_eq!(r.scale, -1);
    }

    #[test]
    fn negate_and_abs() {
        let d = JBigDecimal::from_string(JString::from("5.5"));
        assert_eq!(format!("{}", d.negate()), "-5.5");
        assert_eq!(format!("{}", d.negate().abs()), "5.5");
    }
}
