#![macro_use]

/// Helper macro to generate builder pattern methods for a struct.
///
/// # Example
/// ```
/// pub struct EmitterConfig {
///     pub line_separator: String,
///     pub indent_string: String
/// }
///
/// impl EmitterConfig {
///     pub fn new() -> EmitterConfig {
///         EmitterConfig {
///             line_separator: "\n".to_owned(),
///             indent_string: "  ".to_owned()
///         }
///     }
/// }
///
/// gen_setters!(EmitterConfig,
///     line_separator: String,
///     indent_string: String
/// );
///
/// fn main() {
///     let config = EmitterConfig::new().line_separator("\r\n".to_owned());
/// }
/// ```
macro_rules! gen_setters(
    ($target:ty, $($field:ident : $t:ty),+) => ($(
        impl $target {
            /// Sets the field to the provided value and returns
            /// updated config object.
            pub fn $field(mut self, value: $t) -> $target {
                self.$field = value;
                self
            }
        }
    )+)
);


/// Helper macro for unwrapping a result if possible, continuing the loop
/// if the value is an error.
macro_rules! unwrap_or_continue(
    ($e:expr) => (
        match $e {
            Ok(v) => v,
            _     => { continue; }
        }
    )
);

/// Helper macro for unwrapping an Option if possible, continuing the loop
/// if the value is None.
macro_rules! unwrap_opt_or_continue(
    ($e:expr) => (
        match $e {
            Some(v) => v,
            _       => { continue; }
        }
    )
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
