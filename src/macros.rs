#![macro_use]

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
