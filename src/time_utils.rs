use time::{Timespec, Tm, at_utc};
use std::fmt::{Display, Error, Formatter};

/// Default timespec, used to signal a non initialized time.
pub const DEFAULT_TIMESPEC : Timespec = Timespec{ sec: 0, nsec : 0 };

/// Utility struct that implements Display in pretty way
/// for some Tm instance.
pub struct PrettyDisplay {
    tm : Tm
}

impl Display for PrettyDisplay {
    fn fmt(&self, f : &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.tm.rfc822z())
    }
}

/// Returns an object implementing Display as a pretty printed UTC time.
pub fn to_pretty_utc(ts : Timespec) -> PrettyDisplay {
    PrettyDisplay{ tm : at_utc(ts) }
}

/// Parse a string representing a duplicity timestamp and returns a Timespec
/// if all goes well.
pub fn parse_time_str(s : &str) -> Option<Timespec> {
    use time::strptime;
    strptime(s, "%Y%m%dt%H%M%S%Z").ok().map(|tm| tm.to_timespec())
}
