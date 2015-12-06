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


#[cfg(test)]
mod test {
    use super::*;
    use std::env;
    use time::{self, Tm};


    fn time(y: i32, mon: i32, d: i32, h: i32, min: i32, s: i32) -> Tm {
        Tm {
            tm_sec: s,
            tm_min: min,
            tm_hour: h,
            tm_mday: d,
            tm_mon: mon - 1,
            tm_year: y - 1900,
            tm_wday: 0,
            tm_yday: d,
            tm_isdst: 0,
            tm_utcoff: 0,
            tm_nsec: 0,
        }
    }

    fn this_year() -> i32 {
        time::now_utc().tm_year
    }

    fn move_to_this_year(mut tm: Tm) -> Tm {
        tm.tm_year = this_year();
        tm
    }

    fn set_time_zone(tz: &str) {
        env::set_var("TZ", tz);
        time::tzset();
    }

    #[test]
    fn parse() {
        let time = parse_time_str("19881211t152000z").unwrap();
        let tm = time::at_utc(time);
        assert_eq!(tm.tm_year, 88);
        assert_eq!(tm.tm_mon, 11);  // month in [0 - 11]
        assert_eq!(tm.tm_mday, 11);
        assert_eq!(tm.tm_hour, 15);
        assert_eq!(tm.tm_min, 20);
        assert_eq!(tm.tm_sec, 0);
    }

    #[test]
    fn display_utc() {
        let time = move_to_this_year(time(1988, 12, 11, 15, 20, 0));
        assert_eq!(format!("{}", to_pretty_utc(time.to_timespec())), "Dec 11 15:20");
    }

    // NOTE: changing the time zone is global in the process,
    //       tests are run in parallel,
    //       so, to avoid race conditions in tests requiring a certain timezone you have two
    //       options:
    //       - put every test in the same `test` function
    //       - use a global mutex
    //       we are now using the first option, since the following is the only test requiring a
    //       certain time zone to be set.
    #[test]
    fn display_local() {
        let time = move_to_this_year(time(1988, 12, 11, 15, 20, 0));
        set_time_zone("Europe/London");
        assert_eq!(format!("{}", to_pretty_local(time.to_timespec())), "Dec 11 15:20");
        set_time_zone("Europe/Rome");
        assert_eq!(format!("{}", to_pretty_local(time.to_timespec())), "Dec 11 16:20");
    }

    #[test]
    fn display_past_year() {
        let time = time(1988, 12, 11, 15, 20, 0);
        assert_eq!(format!("{}", to_pretty_utc(time.to_timespec())), "Dec 11  1988");
    }

    #[test]
    fn parse_display_past_year() {
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_utc(time)), "Dec 11  1988");
    }
}
