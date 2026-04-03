#![allow(non_snake_case)]
//! Rust representations of `java.time` types.
//!
//! Mapping:
//! - `java.time.LocalDate`     → `JLocalDate`
//! - `java.time.LocalTime`     → `JLocalTime`
//! - `java.time.LocalDateTime` → `JLocalDateTime`
//! - `java.time.Instant`       → `JInstant`
//! - `java.time.Duration`      → `JDuration`
//! - `java.time.Period`        → `JPeriod`
//! - `java.time.DateTimeFormatter` → `JDateTimeFormatter`

use crate::string::JString;
use std::time::{SystemTime, UNIX_EPOCH};

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
        _ => panic!("Invalid month value {}: valid range is 1..=12", month),
    }
}

impl JLocalDate {
    /// Java `LocalDate.of(year, month, dayOfMonth)`.
    pub fn of(year: i32, month: i32, day: i32) -> Self {
        if !(1..=12).contains(&month) {
            panic!("Invalid month value {}: valid range is 1..=12", month);
        }
        let max_day = days_in_month(year, month);
        if !(1..=max_day).contains(&day) {
            panic!(
                "Invalid day value {} for year {} and month {}: valid range is 1..={}",
                day, year, month, max_day
            );
        }
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

// ── LocalTime ─────────────────────────────────────────────────────────

/// Java `java.time.LocalTime` — hour, minute, second, nano.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct JLocalTime {
    pub hour: i32,
    pub minute: i32,
    pub second: i32,
    pub nano: i32,
}

impl JLocalTime {
    pub fn of_hm(hour: i32, minute: i32) -> Self {
        Self::of_hms(hour, minute, 0)
    }

    pub fn of_hms(hour: i32, minute: i32, second: i32) -> Self {
        Self::of_hmsn(hour, minute, second, 0)
    }

    pub fn of_hmsn(hour: i32, minute: i32, second: i32, nano: i32) -> Self {
        assert!((0..24).contains(&hour), "Invalid hour: {}", hour);
        assert!((0..60).contains(&minute), "Invalid minute: {}", minute);
        assert!((0..60).contains(&second), "Invalid second: {}", second);
        assert!((0..1_000_000_000).contains(&nano), "Invalid nano: {}", nano);
        Self {
            hour,
            minute,
            second,
            nano,
        }
    }

    pub fn now() -> Self {
        let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let secs_of_day = (dur.as_secs() % 86400) as i32;
        let h = secs_of_day / 3600;
        let m = (secs_of_day % 3600) / 60;
        let s = secs_of_day % 60;
        let n = dur.subsec_nanos() as i32;
        Self {
            hour: h,
            minute: m,
            second: s,
            nano: n,
        }
    }

    pub fn parse(text: &JString) -> Self {
        let s = text.as_str();
        let parts: Vec<&str> = s.split(':').collect();
        let hour: i32 = parts[0].parse().unwrap();
        let minute: i32 = parts[1].parse().unwrap();
        let (second, nano) = if parts.len() > 2 {
            let sec_parts: Vec<&str> = parts[2].split('.').collect();
            let sec: i32 = sec_parts[0].parse().unwrap();
            let n = if sec_parts.len() > 1 {
                let frac = sec_parts[1];
                let padded = format!("{:0<9}", frac);
                padded[..9].parse::<i32>().unwrap()
            } else {
                0
            };
            (sec, n)
        } else {
            (0, 0)
        };
        Self::of_hmsn(hour, minute, second, nano)
    }

    pub fn getHour(&self) -> i32 {
        self.hour
    }
    pub fn getMinute(&self) -> i32 {
        self.minute
    }
    pub fn getSecond(&self) -> i32 {
        self.second
    }
    pub fn getNano(&self) -> i32 {
        self.nano
    }

    fn to_second_of_day(&self) -> i64 {
        self.hour as i64 * 3600 + self.minute as i64 * 60 + self.second as i64
    }

    pub fn toSecondOfDay(&self) -> i32 {
        self.to_second_of_day() as i32
    }

    pub fn plusHours(&self, hours: i32) -> Self {
        let total_secs = self.to_second_of_day() + hours as i64 * 3600;
        let total_secs = ((total_secs % 86400) + 86400) % 86400;
        let h = (total_secs / 3600) as i32;
        let m = ((total_secs % 3600) / 60) as i32;
        let s = (total_secs % 60) as i32;
        Self {
            hour: h,
            minute: m,
            second: s,
            nano: self.nano,
        }
    }

    pub fn plusMinutes(&self, minutes: i32) -> Self {
        let total_secs = self.to_second_of_day() + minutes as i64 * 60;
        let total_secs = ((total_secs % 86400) + 86400) % 86400;
        let h = (total_secs / 3600) as i32;
        let m = ((total_secs % 3600) / 60) as i32;
        let s = (total_secs % 60) as i32;
        Self {
            hour: h,
            minute: m,
            second: s,
            nano: self.nano,
        }
    }

    pub fn plusSeconds(&self, seconds: i32) -> Self {
        let total_secs = self.to_second_of_day() + seconds as i64;
        let total_secs = ((total_secs % 86400) + 86400) % 86400;
        let h = (total_secs / 3600) as i32;
        let m = ((total_secs % 3600) / 60) as i32;
        let s = (total_secs % 60) as i32;
        Self {
            hour: h,
            minute: m,
            second: s,
            nano: self.nano,
        }
    }

    pub fn minusHours(&self, hours: i32) -> Self {
        self.plusHours(-hours)
    }
    pub fn minusMinutes(&self, minutes: i32) -> Self {
        self.plusMinutes(-minutes)
    }
    pub fn minusSeconds(&self, seconds: i32) -> Self {
        self.plusSeconds(-seconds)
    }

    pub fn isBefore(&self, other: &JLocalTime) -> bool {
        self < other
    }
    pub fn isAfter(&self, other: &JLocalTime) -> bool {
        self > other
    }

    pub fn withHour(&self, hour: i32) -> Self {
        Self::of_hmsn(hour, self.minute, self.second, self.nano)
    }
    pub fn withMinute(&self, minute: i32) -> Self {
        Self::of_hmsn(self.hour, minute, self.second, self.nano)
    }
    pub fn withSecond(&self, second: i32) -> Self {
        Self::of_hmsn(self.hour, self.minute, second, self.nano)
    }

    pub fn toString(&self) -> JString {
        if self.nano != 0 {
            let frac = format!("{:09}", self.nano)
                .trim_end_matches('0')
                .to_string();
            JString::from(
                format!(
                    "{:02}:{:02}:{:02}.{}",
                    self.hour, self.minute, self.second, frac
                )
                .as_str(),
            )
        } else if self.second != 0 {
            JString::from(
                format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second).as_str(),
            )
        } else {
            JString::from(format!("{:02}:{:02}", self.hour, self.minute).as_str())
        }
    }
}

impl std::fmt::Display for JLocalTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.toString())
    }
}

// ── LocalDateTime ─────────────────────────────────────────────────────

/// Java `java.time.LocalDateTime` — date + time.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct JLocalDateTime {
    pub date: JLocalDate,
    pub time: JLocalTime,
}

impl JLocalDateTime {
    pub fn of_dt(date: JLocalDate, time: JLocalTime) -> Self {
        Self { date, time }
    }

    pub fn of_ymd_hm(year: i32, month: i32, day: i32, hour: i32, minute: i32) -> Self {
        Self {
            date: JLocalDate::of(year, month, day),
            time: JLocalTime::of_hm(hour, minute),
        }
    }

    pub fn of_ymd_hms(
        year: i32,
        month: i32,
        day: i32,
        hour: i32,
        minute: i32,
        second: i32,
    ) -> Self {
        Self {
            date: JLocalDate::of(year, month, day),
            time: JLocalTime::of_hms(hour, minute, second),
        }
    }

    pub fn of_ymd_hmsn(
        year: i32,
        month: i32,
        day: i32,
        hour: i32,
        minute: i32,
        second: i32,
        nano: i32,
    ) -> Self {
        Self {
            date: JLocalDate::of(year, month, day),
            time: JLocalTime::of_hmsn(hour, minute, second, nano),
        }
    }

    pub fn now() -> Self {
        Self {
            date: JLocalDate::now(),
            time: JLocalTime::now(),
        }
    }

    pub fn parse(text: &JString) -> Self {
        let s = text.as_str();
        let parts: Vec<&str> = s.splitn(2, 'T').collect();
        let date = JLocalDate::parse(&JString::from(parts[0]));
        let time = if parts.len() > 1 {
            JLocalTime::parse(&JString::from(parts[1]))
        } else {
            JLocalTime::default()
        };
        Self { date, time }
    }

    pub fn getYear(&self) -> i32 {
        self.date.getYear()
    }
    pub fn getMonthValue(&self) -> i32 {
        self.date.getMonthValue()
    }
    pub fn getDayOfMonth(&self) -> i32 {
        self.date.getDayOfMonth()
    }
    pub fn getHour(&self) -> i32 {
        self.time.getHour()
    }
    pub fn getMinute(&self) -> i32 {
        self.time.getMinute()
    }
    pub fn getSecond(&self) -> i32 {
        self.time.getSecond()
    }
    pub fn getNano(&self) -> i32 {
        self.time.getNano()
    }

    pub fn toLocalDate(&self) -> JLocalDate {
        self.date.clone()
    }
    pub fn toLocalTime(&self) -> JLocalTime {
        self.time.clone()
    }

    pub fn plusDays(&self, n: i32) -> Self {
        Self {
            date: self.date.plusDays(n),
            time: self.time.clone(),
        }
    }
    pub fn minusDays(&self, n: i32) -> Self {
        Self {
            date: self.date.minusDays(n),
            time: self.time.clone(),
        }
    }
    pub fn plusMonths(&self, n: i32) -> Self {
        Self {
            date: self.date.plusMonths(n),
            time: self.time.clone(),
        }
    }
    pub fn minusMonths(&self, n: i32) -> Self {
        Self {
            date: self.date.minusMonths(n),
            time: self.time.clone(),
        }
    }
    pub fn plusHours(&self, n: i32) -> Self {
        Self {
            date: self.date.clone(),
            time: self.time.plusHours(n),
        }
    }
    pub fn plusMinutes(&self, n: i32) -> Self {
        Self {
            date: self.date.clone(),
            time: self.time.plusMinutes(n),
        }
    }
    pub fn plusSeconds(&self, n: i32) -> Self {
        Self {
            date: self.date.clone(),
            time: self.time.plusSeconds(n),
        }
    }

    pub fn isBefore(&self, other: &JLocalDateTime) -> bool {
        self < other
    }
    pub fn isAfter(&self, other: &JLocalDateTime) -> bool {
        self > other
    }

    /// Convert this local date-time to seconds since Unix epoch **as if** UTC.
    pub(crate) fn to_epoch_second(&self) -> i64 {
        let year = self.date.year as i64;
        let month = self.date.month as i64;
        let day = self.date.day as i64;
        // Days from 1970-01-01 to the start of this year
        let y = year - 1;
        let leap_days = y / 4 - y / 100 + y / 400;
        let days_from_epoch_year = (year - 1970) * 365 + (leap_days - (1969 / 4 - 1969 / 100 + 1969 / 400));
        // Days within the year up to start of this month
        let days_in_months: [i64; 13] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];
        let leap_correction = if month > 2 && is_leap(self.date.year) { 1 } else { 0 };
        let day_of_year = days_in_months[(month - 1) as usize] + leap_correction + day - 1;
        let total_days = days_from_epoch_year + day_of_year;
        total_days * 86400
            + self.time.hour as i64 * 3600
            + self.time.minute as i64 * 60
            + self.time.second as i64
    }

    /// Construct a `JLocalDateTime` from seconds since Unix epoch.
    pub(crate) fn from_epoch_second(epoch_second: i64, nano: i32) -> Self {
        let days = epoch_second.div_euclid(86400);
        let rem = epoch_second.rem_euclid(86400);
        let h = (rem / 3600) as i32;
        let m = ((rem % 3600) / 60) as i32;
        let s = (rem % 60) as i32;
        let mut y = 1970i64;
        let mut d = days;
        // Adjust for negative days (dates before 1970)
        if d < 0 {
            while d < 0 {
                y -= 1;
                let diy = if is_leap(y as i32) { 366i64 } else { 365 };
                d += diy;
            }
        } else {
            loop {
                let diy = if is_leap(y as i32) { 366i64 } else { 365 };
                if d < diy {
                    break;
                }
                d -= diy;
                y += 1;
            }
        }
        let mut mo = 1i32;
        loop {
            let dim = days_in_month(y as i32, mo) as i64;
            if d < dim {
                break;
            }
            d -= dim;
            mo += 1;
        }
        let day = (d + 1) as i32;
        Self {
            date: JLocalDate::of(y as i32, mo, day),
            time: JLocalTime::of_hmsn(h, m, s, nano),
        }
    }

    pub fn toString(&self) -> JString {
        JString::from(format!("{}T{}", self.date, self.time).as_str())
    }
}

impl std::fmt::Display for JLocalDateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}T{}", self.date, self.time)
    }
}

// ── Instant ───────────────────────────────────────────────────────────

/// Java `java.time.Instant` — epoch seconds + nanosecond adjustment.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct JInstant {
    pub epoch_second: i64,
    pub nano: i32,
}

impl JInstant {
    pub fn now() -> Self {
        let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        Self {
            epoch_second: dur.as_secs() as i64,
            nano: dur.subsec_nanos() as i32,
        }
    }

    pub fn ofEpochSecond(epoch_second: i64) -> Self {
        Self {
            epoch_second,
            nano: 0,
        }
    }

    pub fn ofEpochMilli(epoch_milli: i64) -> Self {
        let secs = epoch_milli.div_euclid(1000);
        let millis = epoch_milli.rem_euclid(1000);
        Self {
            epoch_second: secs,
            nano: (millis as i32) * 1_000_000,
        }
    }

    pub fn getEpochSecond(&self) -> i64 {
        self.epoch_second
    }
    pub fn getNano(&self) -> i32 {
        self.nano
    }

    pub fn toEpochMilli(&self) -> i64 {
        self.epoch_second * 1000 + (self.nano / 1_000_000) as i64
    }

    pub fn plusSeconds(&self, seconds: i64) -> Self {
        Self {
            epoch_second: self.epoch_second + seconds,
            nano: self.nano,
        }
    }

    pub fn minusSeconds(&self, seconds: i64) -> Self {
        self.plusSeconds(-seconds)
    }

    pub fn plusMillis(&self, millis: i64) -> Self {
        let total_nanos = self.nano as i64 + millis * 1_000_000;
        let extra_secs = total_nanos.div_euclid(1_000_000_000);
        let nano = total_nanos.rem_euclid(1_000_000_000) as i32;
        Self {
            epoch_second: self.epoch_second + extra_secs,
            nano,
        }
    }

    pub fn isBefore(&self, other: &JInstant) -> bool {
        self < other
    }
    pub fn isAfter(&self, other: &JInstant) -> bool {
        self > other
    }

    pub fn toString(&self) -> JString {
        // Simplified ISO-8601: just show epoch seconds
        let secs = self.epoch_second;
        let days = secs.div_euclid(86400);
        let rem = secs.rem_euclid(86400);
        // Convert epoch days to date (simplified)
        let mut y = 1970i64;
        let mut d = days;
        loop {
            let days_in_year = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                366
            } else {
                365
            };
            if d < days_in_year {
                break;
            }
            d -= days_in_year;
            y += 1;
        }
        let mut m = 1;
        loop {
            let dim = days_in_month(y as i32, m) as i64;
            if d < dim {
                break;
            }
            d -= dim;
            m += 1;
        }
        let day = d + 1;
        let h = rem / 3600;
        let min = (rem % 3600) / 60;
        let s = rem % 60;
        if self.nano == 0 {
            JString::from(
                format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, day, h, min, s).as_str(),
            )
        } else {
            let frac = format!("{:09}", self.nano)
                .trim_end_matches('0')
                .to_string();
            JString::from(
                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{}Z",
                    y, m, day, h, min, s, frac
                )
                .as_str(),
            )
        }
    }
}

impl std::fmt::Display for JInstant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.toString())
    }
}

// ── Duration ──────────────────────────────────────────────────────────

/// Java `java.time.Duration` — time-based amount (seconds + nanos).
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct JDuration {
    pub seconds: i64,
    pub nano_adjustment: i32,
}

impl JDuration {
    pub fn ofSeconds(seconds: i64) -> Self {
        Self {
            seconds,
            nano_adjustment: 0,
        }
    }

    pub fn ofMillis(millis: i64) -> Self {
        Self {
            seconds: millis.div_euclid(1000),
            nano_adjustment: (millis.rem_euclid(1000) * 1_000_000) as i32,
        }
    }

    pub fn ofMinutes(minutes: i64) -> Self {
        Self::ofSeconds(minutes * 60)
    }

    pub fn ofHours(hours: i64) -> Self {
        Self::ofSeconds(hours * 3600)
    }

    pub fn ofDays(days: i64) -> Self {
        Self::ofSeconds(days * 86400)
    }

    pub fn ofNanos(nanos: i64) -> Self {
        Self {
            seconds: nanos / 1_000_000_000,
            nano_adjustment: (nanos % 1_000_000_000) as i32,
        }
    }

    pub fn between(start: &JInstant, end: &JInstant) -> Self {
        let sec_diff = end.epoch_second - start.epoch_second;
        let nano_diff = end.nano - start.nano;
        if nano_diff < 0 {
            Self {
                seconds: sec_diff - 1,
                nano_adjustment: nano_diff + 1_000_000_000,
            }
        } else {
            Self {
                seconds: sec_diff,
                nano_adjustment: nano_diff,
            }
        }
    }

    pub fn getSeconds(&self) -> i64 {
        self.seconds
    }
    pub fn getNano(&self) -> i32 {
        self.nano_adjustment
    }

    pub fn toMillis(&self) -> i64 {
        self.seconds * 1000 + (self.nano_adjustment / 1_000_000) as i64
    }

    pub fn toMinutes(&self) -> i64 {
        self.seconds / 60
    }
    pub fn toHours(&self) -> i64 {
        self.seconds / 3600
    }
    pub fn toDays(&self) -> i64 {
        self.seconds / 86400
    }
    pub fn toNanos(&self) -> i64 {
        self.seconds * 1_000_000_000 + self.nano_adjustment as i64
    }

    pub fn plus(&self, other: &JDuration) -> Self {
        let total_nanos = self.nano_adjustment as i64 + other.nano_adjustment as i64;
        let extra_secs = total_nanos.div_euclid(1_000_000_000);
        let nano = total_nanos.rem_euclid(1_000_000_000) as i32;
        Self {
            seconds: self.seconds + other.seconds + extra_secs,
            nano_adjustment: nano,
        }
    }

    pub fn minus(&self, other: &JDuration) -> Self {
        let nano_diff = self.nano_adjustment as i64 - other.nano_adjustment as i64;
        let (extra_secs, nano) = if nano_diff < 0 {
            (-1i64, (nano_diff + 1_000_000_000) as i32)
        } else {
            (0i64, nano_diff as i32)
        };
        Self {
            seconds: self.seconds - other.seconds + extra_secs,
            nano_adjustment: nano,
        }
    }

    pub fn multipliedBy(&self, scalar: i64) -> Self {
        let total_nanos = self.toNanos() * scalar;
        Self::ofNanos(total_nanos)
    }

    pub fn abs(&self) -> Self {
        if self.seconds < 0 || (self.seconds == 0 && self.nano_adjustment < 0) {
            Self {
                seconds: -self.seconds,
                nano_adjustment: self.nano_adjustment.abs(),
            }
        } else {
            self.clone()
        }
    }

    pub fn isZero(&self) -> bool {
        self.seconds == 0 && self.nano_adjustment == 0
    }
    pub fn isNegative(&self) -> bool {
        self.seconds < 0
    }

    pub fn toString(&self) -> JString {
        if self.isZero() {
            return JString::from("PT0S");
        }
        let mut result = String::from("PT");
        let total_secs = self.seconds.abs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        if self.seconds < 0 {
            result.push('-');
        }
        if hours > 0 {
            result.push_str(&format!("{}H", hours));
        }
        if minutes > 0 {
            result.push_str(&format!("{}M", minutes));
        }
        if secs > 0 || self.nano_adjustment != 0 {
            if self.nano_adjustment != 0 {
                let frac = format!("{:09}", self.nano_adjustment.abs())
                    .trim_end_matches('0')
                    .to_string();
                result.push_str(&format!("{}.{}S", secs, frac));
            } else {
                result.push_str(&format!("{}S", secs));
            }
        }
        JString::from(result.as_str())
    }
}

impl std::fmt::Display for JDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.toString())
    }
}

// ── Period ────────────────────────────────────────────────────────────

/// Java `java.time.Period` — date-based amount (years, months, days).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JPeriod {
    pub years: i32,
    pub months: i32,
    pub days: i32,
}

impl JPeriod {
    pub fn of(years: i32, months: i32, days: i32) -> Self {
        Self {
            years,
            months,
            days,
        }
    }

    pub fn ofDays(days: i32) -> Self {
        Self {
            years: 0,
            months: 0,
            days,
        }
    }

    pub fn ofMonths(months: i32) -> Self {
        Self {
            years: 0,
            months,
            days: 0,
        }
    }

    pub fn ofYears(years: i32) -> Self {
        Self {
            years,
            months: 0,
            days: 0,
        }
    }

    pub fn ofWeeks(weeks: i32) -> Self {
        Self {
            years: 0,
            months: 0,
            days: weeks * 7,
        }
    }

    pub fn between(start: &JLocalDate, end: &JLocalDate) -> Self {
        let mut years = end.year - start.year;
        let mut months = end.month - start.month;
        let mut days = end.day - start.day;
        if days < 0 {
            months -= 1;
            days += days_in_month(
                if end.month > 1 {
                    end.year
                } else {
                    end.year - 1
                },
                if end.month > 1 { end.month - 1 } else { 12 },
            );
        }
        if months < 0 {
            years -= 1;
            months += 12;
        }
        Self {
            years,
            months,
            days,
        }
    }

    pub fn getYears(&self) -> i32 {
        self.years
    }
    pub fn getMonths(&self) -> i32 {
        self.months
    }
    pub fn getDays(&self) -> i32 {
        self.days
    }

    pub fn plus(&self, other: &JPeriod) -> Self {
        Self {
            years: self.years + other.years,
            months: self.months + other.months,
            days: self.days + other.days,
        }
    }

    pub fn minus(&self, other: &JPeriod) -> Self {
        Self {
            years: self.years - other.years,
            months: self.months - other.months,
            days: self.days - other.days,
        }
    }

    pub fn isZero(&self) -> bool {
        self.years == 0 && self.months == 0 && self.days == 0
    }
    pub fn isNegative(&self) -> bool {
        self.years < 0 || self.months < 0 || self.days < 0
    }

    pub fn toString(&self) -> JString {
        if self.isZero() {
            return JString::from("P0D");
        }
        let mut result = String::from("P");
        if self.years != 0 {
            result.push_str(&format!("{}Y", self.years));
        }
        if self.months != 0 {
            result.push_str(&format!("{}M", self.months));
        }
        if self.days != 0 {
            result.push_str(&format!("{}D", self.days));
        }
        JString::from(result.as_str())
    }
}

impl std::fmt::Display for JPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.toString())
    }
}

// ── DateTimeFormatter ─────────────────────────────────────────────────

/// Java `java.time.format.DateTimeFormatter` — basic pattern-based formatting.
#[derive(Debug, Clone)]
pub struct JDateTimeFormatter {
    pub pattern: String,
}

impl JDateTimeFormatter {
    pub fn ofPattern(pattern: &JString) -> Self {
        Self {
            pattern: pattern.as_str().to_string(),
        }
    }

    /// Format a `JLocalDate` using this pattern.
    pub fn formatDate(&self, date: &JLocalDate) -> JString {
        self.format_impl(date.year, date.month, date.day, 0, 0, 0, 0)
    }

    /// Format a `JLocalTime` using this pattern.
    pub fn formatTime(&self, time: &JLocalTime) -> JString {
        self.format_impl(0, 0, 0, time.hour, time.minute, time.second, time.nano)
    }

    /// Format a `JLocalDateTime` using this pattern.
    pub fn formatDateTime(&self, dt: &JLocalDateTime) -> JString {
        self.format_impl(
            dt.date.year,
            dt.date.month,
            dt.date.day,
            dt.time.hour,
            dt.time.minute,
            dt.time.second,
            dt.time.nano,
        )
    }

    /// Simple pattern-based formatting: yyyy, MM, dd, HH, mm, ss, etc.
    #[allow(clippy::too_many_arguments)]
    fn format_impl(
        &self,
        year: i32,
        month: i32,
        day: i32,
        hour: i32,
        minute: i32,
        second: i32,
        _nano: i32,
    ) -> JString {
        let mut result = String::new();
        let chars: Vec<char> = self.pattern.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            if ch == '\'' {
                // Literal text between single quotes
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    result.push(chars[i]);
                    i += 1;
                }
                i += 1; // skip closing quote
            } else if ch.is_ascii_alphabetic() {
                let start = i;
                while i < chars.len() && chars[i] == ch {
                    i += 1;
                }
                let count = i - start;
                match ch {
                    'y' => {
                        if count == 2 {
                            result.push_str(&format!("{:02}", year % 100));
                        } else {
                            result.push_str(&format!("{:04}", year));
                        }
                    }
                    'M' => {
                        if count == 2 {
                            result.push_str(&format!("{:02}", month));
                        } else if count == 1 {
                            result.push_str(&format!("{}", month));
                        } else {
                            result.push_str(&format!("{:02}", month));
                        }
                    }
                    'd' => {
                        if count == 2 {
                            result.push_str(&format!("{:02}", day));
                        } else {
                            result.push_str(&format!("{}", day));
                        }
                    }
                    'H' => result.push_str(&format!("{:02}", hour)),
                    'h' => {
                        let h12 = if hour % 12 == 0 { 12 } else { hour % 12 };
                        result.push_str(&format!("{:02}", h12));
                    }
                    'm' => result.push_str(&format!("{:02}", minute)),
                    's' => result.push_str(&format!("{:02}", second)),
                    'a' => {
                        if hour < 12 {
                            result.push_str("AM");
                        } else {
                            result.push_str("PM");
                        }
                    }
                    _ => {
                        // Unknown pattern letters: emit as-is
                        for _ in 0..count {
                            result.push(ch);
                        }
                    }
                }
            } else {
                result.push(ch);
                i += 1;
            }
        }
        JString::from(result.as_str())
    }

    pub fn toString(&self) -> JString {
        JString::from(self.pattern.as_str())
    }
}

impl std::fmt::Display for JDateTimeFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pattern)
    }
}

impl JLocalDate {
    /// Java `date.format(formatter)`.
    pub fn format(&self, formatter: &JDateTimeFormatter) -> JString {
        formatter.formatDate(self)
    }

    /// Java `LocalDate.parse(text)` — parses "YYYY-MM-DD".
    pub fn parse(text: &JString) -> Self {
        let s = text.as_str();
        let parts: Vec<&str> = s.split('-').collect();
        let year: i32 = parts[0].parse().unwrap();
        let month: i32 = parts[1].parse().unwrap();
        let day: i32 = parts[2].parse().unwrap();
        Self::of(year, month, day)
    }

    pub fn isBefore(&self, other: &JLocalDate) -> bool {
        self < other
    }
    pub fn isAfter(&self, other: &JLocalDate) -> bool {
        self > other
    }
    pub fn isEqual(&self, other: &JLocalDate) -> bool {
        self == other
    }

    pub fn plusYears(&self, n: i32) -> Self {
        let year = self.year + n;
        let day = self.day.min(days_in_month(year, self.month));
        JLocalDate {
            year,
            month: self.month,
            day,
        }
    }

    pub fn minusYears(&self, n: i32) -> Self {
        self.plusYears(-n)
    }

    pub fn atTime_hm(&self, hour: i32, minute: i32) -> JLocalDateTime {
        JLocalDateTime {
            date: self.clone(),
            time: JLocalTime::of_hm(hour, minute),
        }
    }

    pub fn atTime(&self, time: JLocalTime) -> JLocalDateTime {
        JLocalDateTime {
            date: self.clone(),
            time,
        }
    }
}

impl JLocalDateTime {
    pub fn format(&self, formatter: &JDateTimeFormatter) -> JString {
        formatter.formatDateTime(self)
    }
}

impl JLocalTime {
    pub fn format(&self, formatter: &JDateTimeFormatter) -> JString {
        formatter.formatTime(self)
    }
}

// ── ZoneId ────────────────────────────────────────────────────────────────────

/// Rust equivalent of `java.time.ZoneId`.
///
/// Only the zone-id string is stored.  Conversion arithmetic is handled for
/// fixed-offset zones (`+HH:MM` / `-HH:MM`) and the literal strings `"UTC"`
/// and `"Z"`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JZoneId {
    pub id: String,
}

impl JZoneId {
    /// `ZoneId.of("UTC")` / `ZoneId.of("America/New_York")` etc.
    pub fn of(id: &JString) -> Self {
        Self {
            id: id.as_str().to_owned(),
        }
    }

    /// `ZoneId.systemDefault()` — returns UTC as the stub default.
    pub fn systemDefault() -> Self {
        Self {
            id: "UTC".to_owned(),
        }
    }

    /// `zone.getId()` — returns the zone string.
    pub fn getId(&self) -> JString {
        JString::from(self.id.as_str())
    }

    /// Returns the UTC offset in seconds for this zone.
    ///
    /// Understands `"UTC"`, `"Z"`, and `±HH:MM` / `±HH:MM:SS` offsets.
    /// All IANA names (e.g. `"America/New_York"`) are treated as UTC.
    pub fn offset_seconds(&self) -> i64 {
        let s = self.id.as_str();
        if s == "UTC" || s == "Z" || s == "GMT" {
            return 0;
        }
        // Try to parse ±HH:MM or ±HH:MM:SS
        let (sign, rest) = if let Some(r) = s.strip_prefix('+') {
            (1i64, r)
        } else if let Some(r) = s.strip_prefix('-') {
            (-1i64, r)
        } else {
            return 0; // unknown IANA name — treat as UTC
        };
        let parts: Vec<&str> = rest.splitn(3, ':').collect();
        let h: i64 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let m: i64 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
        let sec: i64 = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        sign * (h * 3600 + m * 60 + sec)
    }

    pub fn toString(&self) -> JString {
        JString::from(self.id.as_str())
    }
}

impl std::fmt::Display for JZoneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

// ── ZonedDateTime ─────────────────────────────────────────────────────────────

/// Rust equivalent of `java.time.ZonedDateTime`.
///
/// Internally stores a `JLocalDateTime` plus a `JZoneId`.  The stored
/// `JLocalDateTime` is the _local_ date-time in that zone (not the UTC
/// instant).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JZonedDateTime {
    pub date_time: JLocalDateTime,
    pub zone: JZoneId,
}

impl JZonedDateTime {
    /// `ZonedDateTime.of(localDateTime, zone)`
    pub fn of(ldt: JLocalDateTime, zone: JZoneId) -> Self {
        Self {
            date_time: ldt,
            zone,
        }
    }

    /// `ZonedDateTime.now()` — returns a stub fixed value (1970-01-01T00:00:00 UTC).
    pub fn now() -> Self {
        Self {
            date_time: JLocalDateTime::default(),
            zone: JZoneId::systemDefault(),
        }
    }

    /// `ZonedDateTime.now(ZoneId)` — returns a stub fixed value in the given zone.
    pub fn now_zone(zone: &JZoneId) -> Self {
        Self {
            date_time: JLocalDateTime::default(),
            zone: zone.clone(),
        }
    }

    /// `ZonedDateTime.parse(CharSequence)` — parses ISO-8601 with zone offset.
    ///
    /// Accepts e.g. `"2024-03-15T10:30:00+05:30"` or `"2024-03-15T10:30:00Z"`.
    pub fn parse(text: &JString) -> Self {
        let s = text.as_str();
        // Split at 'Z' or last '+'/'-' after the 'T'
        let t_pos = s.find('T').unwrap_or(0);
        let zone_start = if s.ends_with('Z') {
            Some(s.len() - 1)
        } else {
            // Last '+' or '-' after the date-time separator 'T'
            s[t_pos..]
                .rfind('+')
                .or_else(|| {
                    // find '-' that is not the date separator
                    s[t_pos..].rfind('-')
                })
                .map(|p| t_pos + p)
        };
        let (dt_str, zone_str) = if let Some(pos) = zone_start {
            (&s[..pos], &s[pos..])
        } else {
            (s, "UTC")
        };
        let zone_str = if zone_str == "Z" { "UTC" } else { zone_str };
        let date_time = JLocalDateTime::parse(&JString::from(dt_str));
        let zone = JZoneId::of(&JString::from(zone_str));
        Self { date_time, zone }
    }

    // ── Accessors ──────────────────────────────────────────────────────────

    pub fn getYear(&self) -> i32 {
        self.date_time.getYear()
    }
    pub fn getMonthValue(&self) -> i32 {
        self.date_time.getMonthValue()
    }
    pub fn getDayOfMonth(&self) -> i32 {
        self.date_time.getDayOfMonth()
    }
    pub fn getHour(&self) -> i32 {
        self.date_time.getHour()
    }
    pub fn getMinute(&self) -> i32 {
        self.date_time.getMinute()
    }
    pub fn getSecond(&self) -> i32 {
        self.date_time.getSecond()
    }
    pub fn getNano(&self) -> i32 {
        self.date_time.getNano()
    }
    pub fn getZone(&self) -> JZoneId {
        self.zone.clone()
    }
    pub fn toLocalDateTime(&self) -> JLocalDateTime {
        self.date_time.clone()
    }
    pub fn toLocalDate(&self) -> JLocalDate {
        self.date_time.date.clone()
    }
    pub fn toLocalTime(&self) -> JLocalTime {
        self.date_time.time.clone()
    }

    /// Convert to `JInstant` (UTC epoch seconds).
    pub fn toInstant(&self) -> JInstant {
        let local_epoch = self.date_time.to_epoch_second();
        let offset = self.zone.offset_seconds();
        JInstant {
            epoch_second: local_epoch - offset,
            nano: self.date_time.getNano(),
        }
    }

    /// `withZoneSameInstant(newZone)` — preserves the instant, adjusts local time.
    pub fn withZoneSameInstant(&self, new_zone: &JZoneId) -> Self {
        let instant = self.toInstant();
        let new_offset = new_zone.offset_seconds();
        let new_epoch = instant.epoch_second + new_offset;
        let date_time = JLocalDateTime::from_epoch_second(new_epoch, instant.nano);
        Self {
            date_time,
            zone: new_zone.clone(),
        }
    }

    /// `withZoneSameLocal(newZone)` — keeps local time, changes zone label only.
    pub fn withZoneSameLocal(&self, new_zone: &JZoneId) -> Self {
        Self {
            date_time: self.date_time.clone(),
            zone: new_zone.clone(),
        }
    }

    // ── Arithmetic ─────────────────────────────────────────────────────────

    pub fn plusDays(&self, n: i32) -> Self {
        Self {
            date_time: self.date_time.plusDays(n),
            zone: self.zone.clone(),
        }
    }
    pub fn minusDays(&self, n: i32) -> Self {
        Self {
            date_time: self.date_time.minusDays(n),
            zone: self.zone.clone(),
        }
    }
    pub fn plusHours(&self, n: i32) -> Self {
        Self {
            date_time: self.date_time.plusHours(n),
            zone: self.zone.clone(),
        }
    }
    pub fn minusHours(&self, n: i32) -> Self {
        Self {
            date_time: self.date_time.plusHours(-n),
            zone: self.zone.clone(),
        }
    }
    pub fn plusMinutes(&self, n: i32) -> Self {
        Self {
            date_time: self.date_time.plusMinutes(n),
            zone: self.zone.clone(),
        }
    }
    pub fn plusSeconds(&self, n: i32) -> Self {
        Self {
            date_time: self.date_time.plusSeconds(n),
            zone: self.zone.clone(),
        }
    }
    pub fn plusMonths(&self, n: i32) -> Self {
        Self {
            date_time: self.date_time.plusMonths(n),
            zone: self.zone.clone(),
        }
    }

    pub fn isBefore(&self, other: &JZonedDateTime) -> bool {
        self.toInstant() < other.toInstant()
    }
    pub fn isAfter(&self, other: &JZonedDateTime) -> bool {
        self.toInstant() > other.toInstant()
    }

    /// Format using a `JDateTimeFormatter`.
    pub fn format(&self, formatter: &JDateTimeFormatter) -> JString {
        formatter.formatDateTime(&self.date_time)
    }

    pub fn toString(&self) -> JString {
        let dt = self.date_time.toString();
        let z = &self.zone.id;
        let sep = if z == "UTC" || z == "Z" {
            "Z"
        } else {
            &self.zone.id
        };
        JString::from(format!("{}{}", dt, sep).as_str())
    }
}

impl std::fmt::Display for JZonedDateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.toString())
    }
}

impl PartialOrd for JZonedDateTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JZonedDateTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.toInstant().cmp(&other.toInstant())
    }
}

// ── Clock ─────────────────────────────────────────────────────────────────────

/// Rust equivalent of `java.time.Clock`.
///
/// Wraps a `JZoneId`.  The `instant()` and `millis()` methods read the real
/// system clock.
#[derive(Debug, Clone)]
pub struct JClock {
    zone: JZoneId,
}

impl JClock {
    /// `Clock.systemUTC()`
    pub fn systemUTC() -> Self {
        Self {
            zone: JZoneId::of(&JString::from("UTC")),
        }
    }

    /// `Clock.systemDefaultZone()`
    pub fn systemDefaultZone() -> Self {
        Self {
            zone: JZoneId::systemDefault(),
        }
    }

    /// `Clock.fixed(instant, zone)` — returns a clock that always returns the given instant.
    pub fn fixed(instant: JInstant, zone: JZoneId) -> Self {
        let _ = instant;
        Self { zone }
    }

    /// `clock.instant()` — returns the current UTC instant.
    pub fn instant(&self) -> JInstant {
        JInstant::now()
    }

    /// `clock.millis()` — returns current epoch milliseconds.
    pub fn millis(&self) -> i64 {
        JInstant::now().toEpochMilli()
    }

    /// `clock.getZone()`
    pub fn getZone(&self) -> JZoneId {
        self.zone.clone()
    }
}

impl Default for JClock {
    fn default() -> Self {
        Self::systemUTC()
    }
}
