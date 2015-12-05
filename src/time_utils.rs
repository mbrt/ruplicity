use time;
use time::{Timespec, Tm};
use std::fmt::{Display, Error, Formatter};


/// Default timespec, used to signal a non initialized time.
#[allow(dead_code)]
pub const DEFAULT_TIMESPEC: Timespec = Timespec { sec: 0, nsec: 0 };


/// Utility struct that implements Display in a pretty style
/// for some Tm instance.
pub struct PrettyDisplay {
    tm: Tm,
}

/// The format to be used to display a time.
/// It could be a local or an UTC time.
#[allow(dead_code)]
pub enum Format {
    Local,
    Utc,
}

impl Display for PrettyDisplay {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        if time::now_utc().tm_year == self.tm.tm_year {
            // the year is the current, so print month, day, hour
            write!(f, "{}", time::strftime("%b %d %R", &self.tm).unwrap())
        } else {
            // the year is not the current, so print month, day, year
            // NOTE: the double space before year is meaningful
            write!(f, "{}", time::strftime("%b %d  %Y", &self.tm).unwrap())
        }
    }
}


/// Returns an object implementing Display as a pretty printed UTC time.
#[allow(dead_code)]
pub fn to_pretty_utc(ts: Timespec) -> PrettyDisplay {
    PrettyDisplay { tm: time::at_utc(ts) }
}

/// Returns an object implementing Display as a pretty printed local time.
pub fn to_pretty_local(ts: Timespec) -> PrettyDisplay {
    PrettyDisplay { tm: time::at(ts) }
}

/// Returns an obejct implementing Display as a pretty printed time.
/// This could be a local or an UTC time, depending on the format parameter.
#[allow(dead_code)]
pub fn to_pretty(ts: Timespec, format: Format) -> PrettyDisplay {
    match format {
        Format::Local => to_pretty_local(ts),
        Format::Utc => to_pretty_utc(ts),
    }
}



/// Parse a string representing a duplicity timestamp and returns a Timespec
/// if all goes well.
pub fn parse_time_str(s: &str) -> Option<Timespec> {
    time::strptime(s, "%Y%m%dt%H%M%S%Z").ok().map(|tm| tm.to_timespec())
}


/// Test utilities for time
#[cfg(test)]
pub mod test_utils {
    use std::env;
    use std::sync::{Mutex, MutexGuard};
    use time;


    // A global mutex is needed because tests are run in parallel
    // We need to avoid tests to change time zone concurrently
    lazy_static! {
        static ref TZLOCK: Mutex<i32> = Mutex::new(0);
    }

    /// Set the local time zone for the whole process to the given one.
    ///
    /// Returns a `MutexGuard` that avoids other threads to change the time zone concurrently.
    pub fn set_time_zone(tz: &str) -> MutexGuard<i32> {
        let lock = TZLOCK.lock();
        env::set_var("TZ", tz);
        time::tzset();
        lock.unwrap()
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use super::test_utils::set_time_zone;
    use time::{self, Tm};


    fn time(y: i32, mon: i32, d: i32, h: i32, min: i32, s: i32) -> Tm {
        Tm {
            tm_sec: s,
            tm_min: min,
            tm_hour: h,
            tm_mday: d,
            tm_mon: mon,
            tm_year: y,
            tm_wday: 0,
            tm_yday: d,
            tm_isdst: 0,
            tm_utcoff: 0,
            tm_nsec: 0,
        }
    }

    fn today_time(h: i32, m: i32, s: i32) -> Tm {
        let now = time::now_utc();
        time(now.tm_year, now.tm_mon, now.tm_mday, h, m, s)
    }

    #[test]
    fn parse() {
        let time = parse_time_str("19881211t152000z").unwrap();
        let tm = time::at_utc(time);
        assert_eq!(tm.tm_year, 88);
        assert_eq!(tm.tm_mon, 11);
        assert_eq!(tm.tm_mday, 11);
        assert_eq!(tm.tm_hour, 15);
        assert_eq!(tm.tm_min, 20);
        assert_eq!(tm.tm_sec, 0);
    }

    #[test]
    fn display_utc() {
    }

    #[test]
    #[ignore]
    fn parse_display_utc_london() {
        let time = parse_time_str("19881211t152000z").unwrap();
        let _lock = set_time_zone("Europe/London");
        assert_eq!(format!("{}", to_pretty_utc(time)),
                   "Sun, 11 Dec 1988 15:20:00 -0000");
    }

    #[test]
    #[ignore]
    fn parse_display_utc_rome() {
        let _lock = set_time_zone("Europe/Rome");
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_utc(time)),
                   "Sun, 11 Dec 1988 15:20:00 -0000");
    }

    #[test]
    #[ignore]
    fn parse_display_local_rome() {
        let _lock = set_time_zone("Europe/Rome");
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_local(time)),
                   "Sun, 11 Dec 1988 16:20:00 +0100");
    }

    #[test]
    #[ignore]
    fn parse_display_local_london() {
        let _lock = set_time_zone("Europe/London");
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_local(time)),
                   "Sun, 11 Dec 1988 15:20:00 +0100");
    }
}
