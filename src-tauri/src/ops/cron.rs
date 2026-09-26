//! Schedules: standard 5-field cron (`min hour day-of-month month
//! day-of-week`, local time), macros (`@hourly`, `@daily`, `@weekly`,
//! `@monthly`, `@yearly`) and intervals (`@every 15m`, `@every 2h`).
//!
//! Supported field syntax: `*`, `n`, `a-b`, `*/s`, `a-b/s`, `n/s`, lists.
//! Day-of-week is 0–7 (0 and 7 are Sunday). As in Vixie cron, when both
//! day-of-month and day-of-week are restricted, a day matches if *either*
//! does.

use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike};

/// A parsed schedule.
#[derive(Debug, Clone, PartialEq)]
pub enum Schedule {
    /// Cron fields as bitsets.
    Cron {
        /// Minutes 0–59.
        minutes: u64,
        /// Hours 0–23.
        hours: u32,
        /// Days of month 1–31.
        dom: u32,
        /// Months 1–12.
        months: u16,
        /// Days of week 0–6 (Sunday = 0).
        dow: u8,
        /// Day-of-month field was `*`.
        dom_any: bool,
        /// Day-of-week field was `*`.
        dow_any: bool,
    },
    /// Fixed interval.
    Every(Duration),
}

fn field(spec: &str, min: u32, max: u32) -> Result<u64, String> {
    let mut bits = 0u64;
    for part in spec.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (
                r,
                s.parse::<u32>()
                    .ok()
                    .filter(|s| *s > 0)
                    .ok_or_else(|| format!("bad step in `{part}`"))?,
            ),
            None => (part, 1),
        };
        let (lo, hi) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            let a: u32 = a.parse().map_err(|_| format!("bad number in `{part}`"))?;
            let b: u32 = b.parse().map_err(|_| format!("bad number in `{part}`"))?;
            (a, b)
        } else {
            let n: u32 = range.parse().map_err(|_| format!("bad value `{part}`"))?;
            // `n/s` means from n to max in steps of s.
            (n, if part.contains('/') { max } else { n })
        };
        if lo < min || hi > max || lo > hi {
            return Err(format!("`{part}` is outside {min}–{max}"));
        }
        let mut v = lo;
        while v <= hi {
            bits |= 1u64 << v;
            v += step;
        }
    }
    Ok(bits)
}

impl Schedule {
    /// Parse a schedule expression.
    pub fn parse(expr: &str) -> Result<Self, String> {
        let e = expr.trim();
        let e = match e {
            "@hourly" => "0 * * * *",
            "@daily" | "@midnight" => "0 0 * * *",
            "@weekly" => "0 0 * * 0",
            "@monthly" => "0 0 1 * *",
            "@yearly" | "@annually" => "0 0 1 1 *",
            other => other,
        };
        if let Some(rest) = e.strip_prefix("@every") {
            let r = rest.trim();
            let (num, unit) = r.split_at(r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len()));
            let n: i64 = num.parse().map_err(|_| "use @every <n>m|h|d".to_string())?;
            let d = match unit {
                "m" | "min" => Duration::minutes(n),
                "h" => Duration::hours(n),
                "d" => Duration::days(n),
                _ => return Err("use @every <n>m|h|d (minutes, hours, days)".into()),
            };
            if d < Duration::minutes(1) || d > Duration::days(366) {
                return Err("intervals must be between 1 minute and 366 days".into());
            }
            return Ok(Schedule::Every(d));
        }
        let f: Vec<&str> = e.split_whitespace().collect();
        if f.len() != 5 {
            return Err("expected 5 fields: minute hour day-of-month month day-of-week".into());
        }
        let mut dow = field(f[4], 0, 7)? as u8;
        if dow & (1 << 7) != 0 {
            dow = (dow & 0x7f) | 1; // 7 = Sunday
        }
        Ok(Schedule::Cron {
            minutes: field(f[0], 0, 59)?,
            hours: field(f[1], 0, 23)? as u32,
            dom: field(f[2], 1, 31)? as u32,
            months: field(f[3], 1, 12)? as u16,
            dow,
            dom_any: f[2] == "*",
            dow_any: f[4] == "*",
        })
    }

    /// First run strictly after `after` (for `Every`: `after + interval`).
    /// `None` if nothing matches within ~5 years (e.g. `0 0 31 2 *`).
    pub fn next_after(&self, after: DateTime<Local>) -> Option<DateTime<Local>> {
        let (minutes, hours, dom, months, dow, dom_any, dow_any) = match self {
            Schedule::Every(d) => return Some(after + *d),
            Schedule::Cron {
                minutes,
                hours,
                dom,
                months,
                dow,
                dom_any,
                dow_any,
            } => (*minutes, *hours, *dom, *months, *dow, *dom_any, *dow_any),
        };
        let start = after.with_second(0)?.with_nanosecond(0)? + Duration::minutes(1);
        let mut t = start.naive_local();
        let limit = t + Duration::days(366 * 5);
        while t < limit {
            if months & (1 << t.month()) == 0 {
                // jump to the first minute of next month
                let (y, m) = if t.month() == 12 {
                    (t.year() + 1, 1)
                } else {
                    (t.year(), t.month() + 1)
                };
                t = chrono::NaiveDate::from_ymd_opt(y, m, 1)?.and_hms_opt(0, 0, 0)?;
                continue;
            }
            let dom_ok = dom & (1 << t.day()) != 0;
            let dow_ok = dow & (1 << t.weekday().num_days_from_sunday()) != 0;
            let day_ok = match (dom_any, dow_any) {
                (true, true) => true,
                (false, true) => dom_ok,
                (true, false) => dow_ok,
                (false, false) => dom_ok || dow_ok,
            };
            if !day_ok {
                t = (t.date() + Duration::days(1)).and_hms_opt(0, 0, 0)?;
                continue;
            }
            if hours & (1 << t.hour()) == 0 {
                t = t.date().and_hms_opt(t.hour(), 0, 0)? + Duration::hours(1);
                continue;
            }
            if minutes & (1u64 << t.minute()) == 0 {
                t += Duration::minutes(1);
                continue;
            }
            // DST: a nonexistent local time is skipped; an ambiguous one runs once (earliest).
            match Local.from_local_datetime(&t) {
                chrono::LocalResult::Single(d) => return Some(d),
                chrono::LocalResult::Ambiguous(a, _) => return Some(a),
                chrono::LocalResult::None => {
                    t += Duration::minutes(1);
                    continue;
                }
            }
        }
        None
    }

    /// Short human description.
    pub fn describe(expr: &str) -> String {
        match expr.trim() {
            "@hourly" => "every hour".into(),
            "@daily" | "@midnight" => "every day at midnight".into(),
            "@weekly" => "every Sunday at midnight".into(),
            "@monthly" => "on the 1st of every month".into(),
            "@yearly" | "@annually" => "every 1 January".into(),
            e if e.starts_with("@every") => e.trim_start_matches('@').to_string(),
            e => format!("cron `{e}`"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(y, mo, d)
                    .expect("date")
                    .and_hms_opt(h, mi, 0)
                    .expect("time"),
            )
            .earliest()
            .expect("local")
    }

    #[test]
    fn parses_and_rejects() {
        assert!(Schedule::parse("*/15 * * * *").is_ok());
        assert!(Schedule::parse("0 9 * * 1-5").is_ok());
        assert!(Schedule::parse("30 2 1,15 * 7").is_ok());
        assert!(Schedule::parse("@daily").is_ok());
        assert!(Schedule::parse("@every 15m").is_ok());
        for bad in [
            "* * * *",
            "60 * * * *",
            "* 24 * * *",
            "*/0 * * * *",
            "a * * * *",
            "@every 0m",
            "@every 5x",
            "5-2 * * * *",
        ] {
            assert!(Schedule::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn next_runs() {
        let s = Schedule::parse("*/15 * * * *").expect("parse");
        assert_eq!(
            s.next_after(at(2026, 9, 25, 10, 7)),
            Some(at(2026, 9, 25, 10, 15))
        );
        assert_eq!(
            s.next_after(at(2026, 9, 25, 10, 15)),
            Some(at(2026, 9, 25, 10, 30))
        );

        // weekdays at 09:00; 2026-09-25 is a Friday
        let s = Schedule::parse("0 9 * * 1-5").expect("parse");
        assert_eq!(
            s.next_after(at(2026, 9, 25, 9, 0)),
            Some(at(2026, 9, 28, 9, 0))
        );

        let s = Schedule::parse("@monthly").expect("parse");
        assert_eq!(
            s.next_after(at(2026, 12, 15, 0, 0)),
            Some(at(2027, 1, 1, 0, 0))
        );

        // Sunday as 7
        let s = Schedule::parse("0 0 * * 7").expect("parse");
        assert_eq!(
            s.next_after(at(2026, 9, 25, 0, 0)),
            Some(at(2026, 9, 27, 0, 0))
        );

        // dom OR dow when both restricted: the 1st or any Monday
        let s = Schedule::parse("0 0 1 * 1").expect("parse");
        assert_eq!(
            s.next_after(at(2026, 9, 25, 0, 0)),
            Some(at(2026, 9, 28, 0, 0))
        );

        assert_eq!(
            Schedule::parse("0 0 31 2 *")
                .expect("parse")
                .next_after(at(2026, 1, 1, 0, 0)),
            None
        );

        let s = Schedule::parse("@every 2h").expect("parse");
        assert_eq!(
            s.next_after(at(2026, 9, 25, 10, 0)),
            Some(at(2026, 9, 25, 12, 0))
        );
    }
}
