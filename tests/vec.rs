// Tests to understand how some Rust collections work

#[test]
fn double_ended_iter() {
    let v = vec![0, 1, 2, 3];
    let mut iter = v.iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    // returns the end of the range and consume it
    assert_eq!(iter.next_back(), Some(&3));
    assert_eq!(iter.next(), Some(&2));
    // now we are at the end: element 3 was consumed by prev next_back
    assert_eq!(iter.next(), None);
}

#[test]
fn peek() {
    let v = vec![0, 1, 2, 3];
    let mut iter = v.iter().peekable();
    assert_eq!(iter.peek(), Some(&&0));
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.peek(), Some(&&2));
    assert_eq!(iter.peek(), Some(&&2));
}
