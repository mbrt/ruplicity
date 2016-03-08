//! Utilities to parse and display timestamps.
//!
//! This sub-module contains a trait that can be used to display a timestamp in local or UTC time
//! zones, and a function to parse a timestamp.
//!
//! # Example
//! Parse a duplicity timestamp and display it:
//!
//! ```
//! use ruplicity::timefmt::{parse_time_str, TimeDisplay};
//!
//! let time = parse_time_str("19881211t152000z").unwrap();
//! println!("My birth is {}", time.into_local_display());
//! ```

use time;
use time::{Timespec, Tm};
use std::fmt::{Display, Result, Formatter};


/// Trait that allows to display a time into a local or UTC timezone.
pub trait TimeDisplay {
    /// The displayable type
    type D: Display;

    /// Turns self into a displayable type that when displayed uses the local time zone.
    fn into_local_display(self) -> Self::D;
    /// Turns self into a displayable type that when displayed uses the UTC time zone.
    fn into_utc_display(self) -> Self::D;
}

/// Implements `Display` in a pretty style for some Tm instance.
///
/// The format is `month day year` in case the timestamp is for a year different than the current;
/// it is `month day time` otherwise.
///
/// # Example
/// Suppose to have the timestamp `2012-02-22T14:53:18Z`. If we are in 2012, the display will be
/// `Feb 22 14:53`; if we are in 2015, the display will be `Feb 22  2012`.
#[derive(Copy, Clone, Debug)]
pub struct PrettyDisplay(Tm);


/// Parse a string representing a duplicity timestamp and returns a `Timespec` if all goes well.
///
/// An example of such a timestamp is "19881211t152000z" which represents the date
/// `1988-12-11T15:20:00Z` in the UTC time zone.
pub fn parse_time_str(s: &str) -> Option<Timespec> {
    time::strptime(s, "%Y%m%dt%H%M%S%Z").ok().map(|tm| tm.to_timespec())
}


impl TimeDisplay for Timespec {
    type D = PrettyDisplay;

    fn into_local_display(self) -> Self::D {
        PrettyDisplay(time::at(self))
    }

    fn into_utc_display(self) -> Self::D {
        PrettyDisplay(time::at_utc(self))
    }
}


impl Display for PrettyDisplay {
    fn fmt(&self, f: &mut Formatter) -> Result {
        if time::now_utc().tm_year == self.0.tm_year {
            // the year is the current, so print month, day, hour
            write!(f, "{}", time::strftime("%b %d %R", &self.0).unwrap())
        } else {
            // the year is not the current, so print month, day, year
            // NOTE: the double space before year is meaningful
            write!(f, "{}", time::strftime("%b %d  %Y", &self.0).unwrap())
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;
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

    #[cfg(unix)]
    fn set_time_zone(tz: &str) {
        use std::env;
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
        assert_eq!(format!("{}", time.to_timespec().into_utc_display()),
                   "Dec 11 15:20");
    }

    // NOTE: changing the time zone is global in the process,
    //       tests are run in parallel,
    //       so, to avoid race conditions in tests requiring a certain timezone you have two
    //       options:
    //       - put every test in the same `test` function
    //       - use a global mutex
    //       we are now using the first option, since the following is the only test requiring a
    //       certain time zone to be set.
    #[cfg(unix)]
    #[test]
    fn display_local() {
        let time = move_to_this_year(time(1988, 12, 11, 15, 20, 0));
        set_time_zone("Europe/London");
        assert_eq!(format!("{}", time.to_timespec().into_local_display()),
                   "Dec 11 15:20");
        set_time_zone("Europe/Rome");
        assert_eq!(format!("{}", time.to_timespec().into_local_display()),
                   "Dec 11 16:20");
    }

    #[test]
    fn display_past_year() {
        let time = time(1988, 12, 11, 15, 20, 0);
        assert_eq!(format!("{}", time.to_timespec().into_utc_display()),
                   "Dec 11  1988");
    }

    #[test]
    fn parse_display_past_year() {
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", time.into_utc_display()), "Dec 11  1988");
    }
}
