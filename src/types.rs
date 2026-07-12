use std::error::Error as StdError;
use std::fmt;
use std::str::FromStr;

/// The four-component version returned by `MV3D_LP_GetVersion`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SdkVersion {
    major: u32,
    minor: u32,
    patch: u32,
    build: u32,
}

impl SdkVersion {
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32, build: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            build,
        }
    }

    #[must_use]
    pub const fn major(self) -> u32 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u32 {
        self.minor
    }

    #[must_use]
    pub const fn patch(self) -> u32 {
        self.patch
    }

    #[must_use]
    pub const fn build(self) -> u32 {
        self.build
    }

    #[must_use]
    pub const fn components(self) -> [u32; 4] {
        [self.major, self.minor, self.patch, self.build]
    }
}

impl fmt::Display for SdkVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{}.{}.{}",
            self.major, self.minor, self.patch, self.build
        )
    }
}

impl FromStr for SdkVersion {
    type Err = ParseSdkVersionError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let mut components = value.split('.');
        let major = parse_component(components.next())?;
        let minor = parse_component(components.next())?;
        let patch = parse_component(components.next())?;
        let build = parse_component(components.next())?;

        if components.next().is_some() {
            return Err(ParseSdkVersionError);
        }

        Ok(Self::new(major, minor, patch, build))
    }
}

fn parse_component(component: Option<&str>) -> std::result::Result<u32, ParseSdkVersionError> {
    let component = component.filter(|component| !component.is_empty());
    component
        .ok_or(ParseSdkVersionError)?
        .parse()
        .map_err(|_| ParseSdkVersionError)
}

/// Returned when an SDK version is not exactly four decimal components.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseSdkVersionError;

impl fmt::Display for ParseSdkVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected an SDK version in major.minor.patch.build form")
    }
}

impl StdError for ParseSdkVersionError {}
