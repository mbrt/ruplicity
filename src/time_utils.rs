use chrono::{DateTime, Local, NaiveDateTime, TimeZone, UTC};
use std::fmt::{Display, Error, Formatter};

type Timestamp = i64;

/// Default timespec, used to signal a non initialized time.
pub const DEFAULT_TIMESPEC: Timestamp = 0;


/// Utility struct that implements Display in a pretty style
/// for some Tm instance.
pub struct PrettyDisplay<Tz: TimeZone> {
    dt: DateTime<Tz>
}

/// The format to be used to display a time.
/// It could be a local or an UTC time.
#[allow(dead_code)]
pub enum Format {
    Local,
    Utc
}

impl<Tz: TimeZone> Display for PrettyDisplay<Tz> where Tz::Offset: Display {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.dt.to_rfc2822())
    }
}


/// Returns an object implementing Display as a pretty printed UTC time.
#[allow(dead_code)]
pub fn to_pretty_utc(ts: Timestamp) -> PrettyDisplay<UTC> {
    let naive = NaiveDateTime::from_timestamp(ts, 0);
    let dt: DateTime<UTC> = DateTime::from_utc(naive, UTC::Offset);
    PrettyDisplay{ ts: dt }
}

/// Returns an object implementing Display as a pretty printed local time.
pub fn to_pretty_local(ts: Timestamp) -> PrettyDisplay<Local> {
    let naive = NaiveDateTime::from_timestamp(ts, 0);
    let dt: DateTime<Local> = DateTime::from_utc(naive, Local::Offset);
    PrettyDisplay{ ts: dt }
}

/// Parse a string representing a duplicity timestamp and returns a Timestamp
/// if all goes well.
pub fn parse_time_str(s: &str) -> Option<Timestamp> {
    NaiveDateTime::parse_from_str(s, "%Y%m%dt%H%M%S%Z").ok().map(|dt| dt.timestamp())
}
