use crate::domain::i18n::Language;
use std::cmp::Ordering;
use std::env;

#[repr(C)]
#[derive(Clone, Copy)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: *const i8,
}

extern "C" {
    fn time(tloc: *mut i64) -> i64;
    fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Ord for LocalDate {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.year, self.month, self.day).cmp(&(other.year, other.month, other.day))
    }
}

impl PartialOrd for LocalDate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl LocalDate {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Returns today's date in local time, or parses `YARUBE_DATE` / `BUJO_DATE` (YYYY-MM-DD) if set.
    pub fn today() -> Self {
        for key in ["YARUBE_DATE", "BUJO_DATE"] {
            if let Ok(val) = env::var(key) {
                if let Some(parsed) = Self::parse_ymd(val.trim()) {
                    return parsed;
                }
            }
        }
        unsafe {
            let now = time(std::ptr::null_mut());
            let mut tm: Tm = std::mem::zeroed();
            localtime_r(&now, &mut tm);
            Self {
                year: tm.tm_year + 1900,
                month: (tm.tm_mon + 1) as u32,
                day: tm.tm_mday as u32,
            }
        }
    }

    pub fn parse_ymd(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let year = parts[0].parse::<i32>().ok()?;
        let month = parts[1].parse::<u32>().ok()?;
        let day = parts[2].parse::<u32>().ok()?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        Some(Self { year, month, day })
    }

    /// Formats as `YYYY-MM-DD` for file naming.
    pub fn format_ymd(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Formats as `### MM/DD(曜)` matching Sample Information (`### 10/17(火)`).
    pub fn format_header(&self) -> String {
        self.format_header_for(Language::Ja)
    }

    /// Formats the date header according to `lang` (`### 10/17(火)` for Ja, `### 10/17(Tue)` for En).
    pub fn format_header_for(&self, lang: Language) -> String {
        let wday = match (lang, self.weekday_index()) {
            (Language::Ja, 0) => "日",
            (Language::Ja, 1) => "月",
            (Language::Ja, 2) => "火",
            (Language::Ja, 3) => "水",
            (Language::Ja, 4) => "木",
            (Language::Ja, 5) => "金",
            (Language::Ja, _) => "土",
            (Language::En, 0) => "Sun",
            (Language::En, 1) => "Mon",
            (Language::En, 2) => "Tue",
            (Language::En, 3) => "Wed",
            (Language::En, 4) => "Thu",
            (Language::En, 5) => "Fri",
            (Language::En, _) => "Sat",
        };
        format!("### {:02}/{:02}({})", self.month, self.day, wday)
    }

    /// Returns the next calendar day.
    pub fn next_day(&self) -> Self {
        self.add_days(1)
    }

    /// Adds (or subtracts when negative) calendar days.
    pub fn add_days(&self, delta: i64) -> Self {
        let days = self.to_days() + delta;
        Self::from_days(days)
    }

    /// 0 = Sunday, 1 = Monday, ..., 6 = Saturday
    pub fn weekday_index(&self) -> u32 {
        let z = self.to_days();
        ((z + 4).rem_euclid(7)) as u32
    }

    fn to_days(self) -> i64 {
        let y = self.year as i64 - if self.month <= 2 { 1 } else { 0 };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = (y - era * 400) as u64;
        let m = self.month as u64;
        let d = self.day as u64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + (doe as i64) - 719468
    }

    fn from_days(z: i64) -> Self {
        let z = z + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = (z - era * 146097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = (yoe as i64) + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
        Self {
            year,
            month: m as u32,
            day: d as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_format_ymd() {
        let d = LocalDate::parse_ymd("2026-10-20").unwrap();
        assert_eq!(d, LocalDate::new(2026, 10, 20));
        assert_eq!(d.format_ymd(), "2026-10-20");
        assert_eq!(d.format_header(), "### 10/20(火)");
    }

    #[test]
    fn test_next_day_and_ordering() {
        let d1 = LocalDate::new(2026, 10, 31);
        let d2 = d1.next_day();
        assert_eq!(d2, LocalDate::new(2026, 11, 1));
        assert!(d1 < d2);
        assert_eq!(d2.add_days(-1), d1);
    }
}
