use crate::isl::isl_range::{Range, RangeType};
use crate::isl::{IslVersion, WriteToIsl};
use crate::result::{invalid_schema_error, IonSchemaError, IonSchemaResult};
use ion_rs::element::writer::ElementWriter;
use ion_rs::element::Element;
use ion_rs::types::Precision;
use ion_rs::{IonWriter, Symbol, Timestamp};
use num_traits::abs;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::{Display, Formatter};

/// Represents an annotation for `annotations` constraint.
/// ```ion
/// Grammar: <ANNOTATION> ::= <SYMBOL>
///                | required::<SYMBOL>
///                | optional::<SYMBOL>
/// ```
/// `annotations`: `<https://amazon-ion.github.io/ion-schema/docs/isl-1-0/spec#annotations>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    isl_version: IslVersion,
    value: String,
    is_required: bool, // Specifies whether an annotation's occurrence is required or optional
}

impl Annotation {
    pub fn new(value: String, is_required: bool, isl_version: IslVersion) -> Self {
        Self {
            value,
            is_required,
            isl_version,
        }
    }

    pub fn value(&self) -> &String {
        &self.value
    }

    pub fn is_required(&self) -> bool {
        self.is_required
    }

    // Returns a bool value that represents if an annotation is required or not
    pub(crate) fn is_annotation_required(value: &Element, list_level_required: bool) -> bool {
        if value.annotations().contains("required") {
            true
        } else if list_level_required {
            // if the value is annotated with `optional` then it overrides the list-level `required` behavior
            !value.annotations().contains("optional")
        } else {
            // for any value the default annotation is `optional`
            false
        }
    }
}

impl WriteToIsl for Annotation {
    fn write_to<W: IonWriter>(&self, writer: &mut W) -> IonSchemaResult<()> {
        if self.isl_version == IslVersion::V1_0 {
            if self.is_required {
                writer.set_annotations(["required"]);
            } else {
                writer.set_annotations(["optional"]);
            }
        }
        writer.write_symbol(&self.value)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampPrecision {
    Year,
    Month,
    Day,
    Minute,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
    OtherFractionalSeconds(i64),
}

impl TimestampPrecision {
    pub fn from_timestamp(timestamp_value: &Timestamp) -> TimestampPrecision {
        use TimestampPrecision::*;
        let precision_value = timestamp_value.precision();
        match precision_value {
            Precision::Year => Year,
            Precision::Month => Month,
            Precision::Day => Day,
            Precision::HourAndMinute => Minute,
            // `unwrap_or(0)` is a default to set second as timestamp precision.
            // currently `fractional_seconds_scale` doesn't return 0 for Precision::Second,
            // once that is fixed in ion-rust we can remove unwrap_or from here
            Precision::Second => match timestamp_value.fractional_seconds_scale().unwrap_or(0) {
                0 => Second,
                3 => Millisecond,
                6 => Microsecond,
                9 => Nanosecond,
                scale => OtherFractionalSeconds(scale),
            },
        }
    }
}

impl TryFrom<&str> for TimestampPrecision {
    type Error = IonSchemaError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "year" => TimestampPrecision::Year,
            "month" => TimestampPrecision::Month,
            "day" => TimestampPrecision::Day,
            "minute" | "hour" => TimestampPrecision::Minute,
            "second" => TimestampPrecision::Second,
            "millisecond" => TimestampPrecision::Millisecond,
            "microsecond" => TimestampPrecision::Microsecond,
            "nanosecond" => TimestampPrecision::Nanosecond,
            _ => {
                return invalid_schema_error(format!(
                    "Invalid timestamp precision specified {value}"
                ))
            }
        })
    }
}

impl PartialOrd for TimestampPrecision {
    fn partial_cmp(&self, other: &TimestampPrecision) -> Option<Ordering> {
        use TimestampPrecision::*;
        let self_value = match self {
            Year => -4,
            Month => -3,
            Day => -2,
            Minute => -1,
            Second => 0,
            Millisecond => 3,
            Microsecond => 6,
            Nanosecond => 9,
            OtherFractionalSeconds(scale) => *scale,
        };

        let other_value = match other {
            Year => -4,
            Month => -3,
            Day => -2,
            Minute => -1,
            Second => 0,
            Millisecond => 3,
            Microsecond => 6,
            Nanosecond => 9,
            OtherFractionalSeconds(scale) => *scale,
        };

        Some(self_value.cmp(&other_value))
    }
}

impl Display for TimestampPrecision {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self {
            TimestampPrecision::Year => write!(f, "year"),
            TimestampPrecision::Month => write!(f, "month"),
            TimestampPrecision::Day => write!(f, "day"),
            TimestampPrecision::Minute => write!(f, "minute"),
            TimestampPrecision::Second => write!(f, "second"),
            TimestampPrecision::Millisecond => write!(f, "millisecond"),
            TimestampPrecision::Microsecond => write!(f, "microsecond"),
            TimestampPrecision::Nanosecond => write!(f, "nanosecond"),
            TimestampPrecision::OtherFractionalSeconds(scale) => {
                write!(f, "fractional second (10e{})", scale * -1)
            }
        }
    }
}

/// Represents a valid value to be ued within `valid_values` constraint
/// ValidValue could either be a range or Element
/// ```ion
/// Grammar: <VALID_VALUE> ::= <VALUE>
///                | <RANGE<TIMESTAMP>>
///                | <RANGE<NUMBER>>
/// ```
/// `valid_values`: `<https://amazon-ion.github.io/ion-schema/docs/isl-1-0/spec#valid_values>`
#[derive(Debug, Clone, PartialEq)]
pub enum ValidValue {
    Range(Range),
    Element(Element),
}

impl ValidValue {
    pub fn from_ion_element(value: &Element, isl_version: IslVersion) -> IonSchemaResult<Self> {
        if value.annotations().contains("range") {
            Ok(ValidValue::Range(Range::from_ion_element(
                value,
                RangeType::NumberOrTimestamp,
                isl_version,
            )?))
        } else if value
            .annotations()
            .iter()
            .any(|a| a != &Symbol::from("range"))
        {
            invalid_schema_error(
                "Annotations are not allowed for valid_values constraint except `range` annotation",
            )
        } else {
            Ok(ValidValue::Element(value.to_owned()))
        }
    }
}

impl Display for ValidValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            ValidValue::Range(range) => write!(f, "{range}"),
            ValidValue::Element(element) => write!(f, "{element}"),
        }
    }
}

impl WriteToIsl for ValidValue {
    fn write_to<W: IonWriter>(&self, writer: &mut W) -> IonSchemaResult<()> {
        match self {
            ValidValue::Range(range) => range.write_to(writer)?,
            ValidValue::Element(element) => writer.write_element(element)?,
        }
        Ok(())
    }
}

/// Represent a timestamp offset
/// Known timestamp offset value is stored in minutes as i32 value
/// For example, "+07::00" wil be stored as `TimestampOffset::Known(420)`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampOffset {
    Known(i32), // represents known timestamp offset in minutes
    Unknown,    // represents unknown timestamp offset "-00:00"
}

impl TryFrom<&str> for TimestampOffset {
    type Error = IonSchemaError;

    fn try_from(string_value: &str) -> Result<Self, Self::Error> {
        // unknown offset will be stored as None
        if string_value == "-00:00" {
            Ok(TimestampOffset::Unknown)
        } else {
            if string_value.len() != 6 || string_value.chars().nth(3).unwrap() != ':' {
                return invalid_schema_error(
                    "`timestamp_offset` values must be of the form \"[+|-]hh:mm\"",
                );
            }
            // convert string offset value into an i32 value of offset in minutes
            let h = &string_value[1..3];
            let m = &string_value[4..6];
            let sign = match &string_value[..1] {
                "-" => -1,
                "+" => 1,
                _ => {
                    return invalid_schema_error(format!(
                        "unrecognized `timestamp_offset` sign '{}'",
                        &string_value[..1]
                    ))
                }
            };
            match (h.parse::<i32>(), m.parse::<i32>()) {
                (Ok(hours), Ok(minutes))
                    if (0..24).contains(&hours) && (0..60).contains(&minutes) =>
                {
                    Ok(TimestampOffset::Known(sign * (hours * 60 + minutes)))
                }
                _ => invalid_schema_error(format!("invalid timestamp offset {string_value}")),
            }
        }
    }
}

impl From<Option<i32>> for TimestampOffset {
    fn from(value: Option<i32>) -> Self {
        use TimestampOffset::*;
        match value {
            None => Unknown,
            Some(offset_in_minutes) => Known(offset_in_minutes),
        }
    }
}

impl Display for TimestampOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use TimestampOffset::*;
        match &self {
            Unknown => write!(f, "-00:00"),
            Known(offset) => {
                let sign = if offset < &0 { "-" } else { "+" };
                let hours = abs(*offset) / 60;
                let minutes = abs(*offset) - hours * 60;
                write!(f, "{sign}{hours:02}:{minutes:02}")
            }
        }
    }
}

impl WriteToIsl for TimestampOffset {
    fn write_to<W: IonWriter>(&self, writer: &mut W) -> IonSchemaResult<()> {
        match &self {
            TimestampOffset::Known(offset) => {
                let sign = if offset < &0 { "-" } else { "+" };
                let hours = abs(*offset) / 60;
                let minutes = abs(*offset) - hours * 60;
                writer.write_string(format!("{sign}{hours:02}:{minutes:02}"))?;
            }
            TimestampOffset::Unknown => writer.write_string("-00:00")?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ieee754InterchangeFormat {
    Binary16,
    Binary32,
    Binary64,
}

impl TryFrom<&str> for Ieee754InterchangeFormat {
    type Error = IonSchemaError;
    fn try_from(string_value: &str) -> Result<Self, Self::Error> {
        use Ieee754InterchangeFormat::*;
        match string_value {
            "binary16" => Ok(Binary16),
            "binary32" => Ok(Binary32),
            "binary64" => Ok(Binary64),
            _ => invalid_schema_error(format!(
                "unrecognized `ieee754_float` value {}",
                &string_value
            )),
        }
    }
}

impl Display for Ieee754InterchangeFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Ieee754InterchangeFormat::Binary16 => "binary16",
                Ieee754InterchangeFormat::Binary32 => "binary32",
                Ieee754InterchangeFormat::Binary64 => "binary64",
            }
        )
    }
}

impl WriteToIsl for Ieee754InterchangeFormat {
    fn write_to<W: IonWriter>(&self, writer: &mut W) -> IonSchemaResult<()> {
        writer.write_symbol(match self {
            Ieee754InterchangeFormat::Binary16 => "binary16",
            Ieee754InterchangeFormat::Binary32 => "binary32",
            Ieee754InterchangeFormat::Binary64 => "binary64",
        })?;
        Ok(())
    }
}
