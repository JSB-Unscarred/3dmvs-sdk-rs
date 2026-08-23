macro_rules! bit_newtype {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        $($const:ident = $value:expr => $label:expr),+ $(,)?
    ) => {
        bit_newtype! {
            @base
            $(#[$meta])*
            $vis struct $name;
            $($const = $value => $label),+
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
    // 状态码类取值的名字直接对照厂商头文件，Display 额外带十六进制位模式。
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident(hex display);
        $($const:ident = $value:expr => $label:expr),+ $(,)?
    ) => {
        bit_newtype! {
            @base
            $(#[$meta])*
            $vis struct $name;
            $($const = $value => $label),+
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.name() {
                    Some(name) => write!(formatter, "{name} (0x{:08X})", self.0),
                    None => write!(
                        formatter,
                        concat!("unknown ", stringify!($name), " 0x{:08X}"),
                        self.0
                    ),
                }
            }
        }
    };
    (
        @base
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        $($const:ident = $value:expr => $label:expr),+
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Eq, Hash, PartialEq)]
        $vis struct $name(u32);

        impl $name {
            $(
                #[doc = concat!("SDK 值 `", stringify!($value), "`，显示为 “", $label, "”。")]
                pub const $const: Self = Self($value);
            )+

            /// 按位保留 SDK 返回的 32 位值；未知值同样可表示。
            #[must_use]
            pub const fn from_raw(raw: i32) -> Self {
                Self(raw.cast_unsigned())
            }

            /// 还原成 SDK 头文件里的有符号取值。
            #[must_use]
            pub const fn raw(self) -> i32 {
                self.0.cast_signed()
            }

            /// 返回该取值的可读名字；来自更新版本 SDK 的未知值返回 `None`。
            #[must_use]
            pub const fn name(self) -> Option<&'static str> {
                $(if self.0 == Self::$const.0 {
                    return Some($label);
                })+
                None
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
    };
}

pub(crate) use bit_newtype;
