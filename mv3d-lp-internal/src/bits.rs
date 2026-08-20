macro_rules! bit_newtype {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        $($const:ident = $value:expr => $label:expr),+ $(,)?
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Eq, Hash, PartialEq)]
        $vis struct $name(u32);

        impl $name {
            $(pub const $const: Self = Self($value);)+

            #[must_use]
            pub const fn from_raw(raw: i32) -> Self {
                Self(raw as u32)
            }

            #[must_use]
            pub const fn from_bits(bits: u32) -> Self {
                Self(bits)
            }

            #[must_use]
            pub const fn raw(self) -> i32 {
                self.0 as i32
            }

            #[must_use]
            pub const fn bits(self) -> u32 {
                self.0
            }

            #[must_use]
            pub const fn name(self) -> Option<&'static str> {
                match self.0 {
                    $($value => Some($label),)+
                    _ => None,
                }
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.name() {
                    Some(name) => {
                        write!(
                            formatter,
                            "{}({}, 0x{:08X})",
                            stringify!($name),
                            name,
                            self.0
                        )
                    }
                    None => {
                        write!(
                            formatter,
                            concat!(stringify!($name), "(0x{:08X})"),
                            self.0
                        )
                    }
                }
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.name() {
                    Some(name) => formatter.write_str(name),
                    None => write!(
                        formatter,
                        concat!("unknown ", stringify!($name), " 0x{:08X}"),
                        self.0
                    ),
                }
            }
        }
    };
}

pub(crate) use bit_newtype;
