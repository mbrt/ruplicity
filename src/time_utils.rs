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


#[cfg(test)]
mod test {
    use super::*;
    use std::env;
    use time;


    #[test]
    fn parse() {
        parse_time_str("19881211t172000z").unwrap();
    }

    #[test]
    fn parse_display_utc() {
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_utc(time)), "Sun, 11 Dec 1988 15:20:00 -0000");
    }

    #[test]
    fn parse_display_local() {
        env::set_var("TZ", "Europe/Rome");
        time::tzset();
        let time = parse_time_str("19881211t152000z").unwrap();
        assert_eq!(format!("{}", to_pretty_local(time)), "Sun, 11 Dec 1988 16:20:00 +0100");
    }
}
