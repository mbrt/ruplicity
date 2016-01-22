extern crate time;
use time::{Tm, at_utc, strftime, strptime};

#[test]
fn time_crate() {
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
