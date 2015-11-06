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
        write!(f, "{}", self.tm.rfc822z())
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


    #[test]
    fn parse() {
        parse_time_str("19881211t172000z").unwrap();
    }

    #[test]
    fn parse_display_utc_london() {
        let time = parse_time_str("19881211t152000z").unwrap();
        let _lock = set_time_zone("Europe/London");
        assert_eq!(format!("{}", to_pretty_utc(time)),
                   "Sun, 11 Dec 1988 15:20:00 -0000");
    }

    #[test]
    fn parse_display_utc_rome() {
        let _lock = set_time_zone("Europe/Rome");
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_utc(time)),
                   "Sun, 11 Dec 1988 15:20:00 -0000");
    }

    #[test]
    fn parse_display_local_rome() {
        let _lock = set_time_zone("Europe/Rome");
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_local(time)),
                   "Sun, 11 Dec 1988 16:20:00 +0100");
    }

    #[test]
    fn parse_display_local_london() {
        let _lock = set_time_zone("Europe/London");
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_local(time)),
                   "Sun, 11 Dec 1988 15:20:00 -0000");
    }

    #[test]
    fn time_crate() {
        use time::{Tm, at_utc, strftime, strptime};

        let _lock = set_time_zone("Europe/Rome");
        // parse
        let tm = strptime("20150617t182545z", "%Y%m%dt%H%M%S%Z").unwrap();
        // format
        assert_eq!(strftime("%a %d/%m/%Y %H:%M:%S", &tm).unwrap(),
                   "Sun 17/06/2015 18:25:45");
        assert_eq!(format!("{}", tm.rfc3339()), "2015-06-17T18:25:45Z");
        // store in Timespec and restore in Tm
        let ts = tm.to_timespec();
        let tm1 = at_utc(ts);
        // somehow they don't have the same identical structure :(
        // tm_wday, tm_yday are missing. See rust-lang-deprecated/time#92
        // assert_eq!(tm, tm1);
        // test equally formatted
        let format_fn = |tm: &Tm| format!("{}", tm.rfc3339());
        assert_eq!(format_fn(&tm), format_fn(&tm1));
    }
}
