#![macro_use]

/// Helper macro for unwrapping a result if possible, continuing the loop
/// if the value is an error.
macro_rules! unwrap_or_continue(
    ($e:expr) => (
        match $e {
            Ok(v) => v,
            _ => { continue; }
        }
    )
);

/// Helper macro for unwrapping an Option if possible, continuing the loop
/// if the value is None.
macro_rules! unwrap_opt_or_continue(
    ($e:expr) => (
        match $e {
            Some(v) => v,
            _ => { continue; }
        }
    )
);

/// Helper macro for unwrapping a Result if possible, returns the given error otherwise.
macro_rules! try_or(
    ($e:expr, $err:expr) => (
        match $e {
            Ok(v) => v,
            _ => { return $err; }
        }
    )
);

/// Helper macro for unwrapping a Result if possible, returns an `fmt::Error` otherwise.
macro_rules! try_or_fmt_err(
    ($e:expr) => (try_or!($e, Err(fmt::Error)))
);

/// Helper macro that tests if the given expression matches a given pattern.
///
/// # Example
/// ```
/// enum E { First, Second }
///
/// fn foo {
///     let v: Vec<E> = Vec::new();
///     v.filter(|e| matches!(e, First));
/// }
/// ```
macro_rules! matches(
    ($e:expr, $p:pat) => (
        match $e {
            $p => true,
            _ => false
        }
    )
);
