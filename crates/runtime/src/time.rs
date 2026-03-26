#![allow(non_snake_case)]
//! [`JLocalDate`] — Rust representation of `java.time.LocalDate`.
//!
//! Mapping: `java.time.LocalDate` → `JLocalDate` (year/month/day triple).
//! Implements Proleptic Gregorian calendar arithmetic.

use crate::string::JString;

/// Java `java.time.LocalDate` — an immutable date (year, month, day).
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct JLocalDate {
    pub year: i32,
    pub month: i32,
    pub day: i32,
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

impl JLocalDate {
    /// Java `LocalDate.of(year, month, dayOfMonth)`.
    pub fn of(year: i32, month: i32, day: i32) -> Self {
        Self { year, month, day }
    }

    /// Java `LocalDate.now()` — returns a fixed epoch date (non-deterministic; use only in non-differential tests).
    pub fn now() -> Self {
        // Returns 1970-01-01 as a deterministic stub.
        // For real usage in generated code, the Java test should capture the date before translating.
        Self {
            year: 1970,
            month: 1,
            day: 1,
        }
    }

    /// Java `date.getYear()`.
    pub fn getYear(&self) -> i32 {
        self.year
    }

    /// Java `date.getMonthValue()`.
    pub fn getMonthValue(&self) -> i32 {
        self.month
    }

    /// Java `date.getDayOfMonth()`.
    pub fn getDayOfMonth(&self) -> i32 {
        self.day
    }

    /// Java `date.getDayOfYear()`.
    pub fn getDayOfYear(&self) -> i32 {
        let mut day_of_year = self.day;
        for m in 1..self.month {
            day_of_year += days_in_month(self.year, m);
        }
        day_of_year
    }

    /// Java `date.plusDays(n)`.
    pub fn plusDays(&self, n: i32) -> JLocalDate {
        let mut year = self.year;
        let mut month = self.month;
        let mut day = self.day + n;

        if n >= 0 {
            loop {
                let dim = days_in_month(year, month);
                if day <= dim {
                    break;
                }
                day -= dim;
                month += 1;
                if month > 12 {
                    month = 1;
                    year += 1;
                }
            }
        } else {
            while day <= 0 {
                month -= 1;
                if month <= 0 {
                    month = 12;
                    year -= 1;
                }
                day += days_in_month(year, month);
            }
        }
        JLocalDate { year, month, day }
    }

    /// Java `date.minusDays(n)`.
    pub fn minusDays(&self, n: i32) -> JLocalDate {
        self.plusDays(-n)
    }

    /// Java `date.plusMonths(n)`.
    pub fn plusMonths(&self, n: i32) -> JLocalDate {
        let total_months = self.month + n - 1;
        let year = self.year + total_months.div_euclid(12);
        let month = total_months.rem_euclid(12) + 1;
        let day = self.day.min(days_in_month(year, month));
        JLocalDate { year, month, day }
    }

    /// Java `date.minusMonths(n)`.
    pub fn minusMonths(&self, n: i32) -> JLocalDate {
        self.plusMonths(-n)
    }

    /// Java `date.withDayOfMonth(day)`.
    pub fn withDayOfMonth(&self, day: i32) -> JLocalDate {
        JLocalDate {
            year: self.year,
            month: self.month,
            day,
        }
    }

    /// Java `date.toString()` — returns "YYYY-MM-DD".
    pub fn toString(&self) -> JString {
        JString::from(format!("{:04}-{:02}-{:02}", self.year, self.month, self.day).as_str())
    }
}

impl std::fmt::Display for JLocalDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}
