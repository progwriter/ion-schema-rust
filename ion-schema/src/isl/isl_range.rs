use crate::isl::isl_range::RangeBoundaryValue::*;
use crate::isl::util::TimestampPrecision;
use crate::isl::IslVersion;
use crate::isl::WriteToIsl;
use crate::result::{
    invalid_schema_error, invalid_schema_error_raw, IonSchemaError, IonSchemaResult,
};
use ion_rs::element::writer::ElementWriter;
use ion_rs::element::Element;
use ion_rs::external::bigdecimal::num_bigint::BigInt;
use ion_rs::external::bigdecimal::{BigDecimal, One};
use ion_rs::types::IntAccess;
use ion_rs::{element, Decimal, Int, IonType, IonWriter, Timestamp};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::prelude::rust_2021::TryInto;
use std::str::FromStr;

/// Provides a type to be used to create integer ranges.
pub type IntegerRange = RangeImpl<Int>;

/// Provides a type to be used to create non negative integer ranges.
pub type NonNegativeIntegerRange = RangeImpl<usize>;

/// Provides a type to be used to create float ranges.
pub type FloatRange = RangeImpl<f64>;

/// Provides a type to be used to create decimal ranges.
pub type DecimalRange = RangeImpl<Decimal>;

/// Provides a type to be used to create timestamp ranges.
pub type TimestampRange = RangeImpl<Timestamp>;

/// Provides a type to be used to create timestamp precision ranges.
pub type TimestampPrecisionRange = RangeImpl<TimestampPrecision>;

/// Provides a type to be used to create number ranges.
pub type NumberRange = RangeImpl<Number>;

/// Represents ISL [Range]s where some constraints can be defined using a range
/// ```ion
/// <RANGE<RANGE_TYPE>> ::= range::[ <EXCLUSIVITY><RANGE_TYPE>, <EXCLUSIVITY><RANGE_TYPE> ]
///                       | range::[ min, <EXCLUSIVITY><RANGE_TYPE> ]
///                       | range::[ <EXCLUSIVITY><RANGE_TYPE>, max ]
/// Grammar: <RANGE_TYPE> ::= <DECIMAL>
///                | <FLOAT>
///                | <INT>
///                | <NUMBER>
///                | <TIMESTAMP>
///                | <TIMESTAMP_PRECISION_VALUE>
/// ```
/// For more information on [Range]: <https://amazon-ion.github.io/ion-schema/docs/isl-1-0/spec#constraints>
// this is a wrapper around the RangeImpl generic implementation of ranges
#[derive(Debug, Clone, PartialEq)]
pub enum Range {
    Integer(RangeImpl<Int>),
    NonNegativeInteger(RangeImpl<usize>),
    TimestampPrecision(RangeImpl<TimestampPrecision>),
    Timestamp(RangeImpl<Timestamp>),
    Decimal(RangeImpl<Decimal>),
    Float(RangeImpl<f64>),
    Number(RangeImpl<Number>),
}

impl Range {
    pub fn non_negative_range_boundaries(&self) -> Option<(usize, usize)> {
        match self {
            Range::NonNegativeInteger(RangeImpl { start, end }) => {
                let start = match start {
                    RangeBoundaryValue::Max => usize::MAX,
                    RangeBoundaryValue::Min => usize::MIN,
                    RangeBoundaryValue::Value(val, range_boundary_type) => {
                        match range_boundary_type {
                            RangeBoundaryType::Inclusive => *val,
                            RangeBoundaryType::Exclusive => *val + 1,
                        }
                    }
                };
                let end = match end {
                    RangeBoundaryValue::Max => usize::MAX,
                    RangeBoundaryValue::Min => usize::MIN,
                    RangeBoundaryValue::Value(val, range_boundary_type) => {
                        match range_boundary_type {
                            RangeBoundaryType::Inclusive => *val,
                            RangeBoundaryType::Exclusive => *val + 1,
                        }
                    }
                };
                Some((start, end))
            }
            _ => None,
        }
    }

    /// Provides a boolean value to specify whether the given value is within the range or not
    pub fn contains(&self, value: &Element) -> bool {
        if value.is_null() {
            // if the provided Element is null, then return false
            return false;
        }
        match self {
            Range::Integer(int_range) if value.ion_type() == IonType::Int => {
                int_range.contains(value.as_int().unwrap().to_owned())
            }
            Range::NonNegativeInteger(int_non_neg_range) if value.ion_type() == IonType::Int => {
                int_non_neg_range.contains(value.as_int().unwrap().as_i64().unwrap() as usize)
            }
            Range::TimestampPrecision(timestamp_precision_range)
                if value.ion_type() == IonType::Timestamp =>
            {
                let value = TimestampPrecision::from_timestamp(value.as_timestamp().unwrap());
                timestamp_precision_range.contains(value)
            }
            Range::Timestamp(timestamp_range) if value.ion_type() == IonType::Timestamp => {
                timestamp_range.contains(value.as_timestamp().unwrap().to_owned())
            }
            Range::Float(float_range) if value.ion_type() == IonType::Float => {
                float_range.contains(value.as_float().unwrap())
            }
            Range::Decimal(decimal_range) if value.ion_type() == IonType::Decimal => {
                decimal_range.contains(value.as_decimal().unwrap().to_owned())
            }
            Range::Number(number_range)
                if value.ion_type() == IonType::Int
                    || value.ion_type() == IonType::Float
                    || value.ion_type() == IonType::Decimal =>
            {
                let value: Number = match value.ion_type() {
                    IonType::Int => value.as_int().unwrap().into(),
                    IonType::Float => {
                        if let Ok(number_val) = value.as_float().unwrap().try_into() {
                            number_val
                        } else {
                            return false;
                        }
                    }
                    IonType::Decimal => {
                        if let Ok(number_val) = value.as_decimal().unwrap().try_into() {
                            number_val
                        } else {
                            return false;
                        }
                    }
                    _ => {
                        return false;
                    }
                };
                number_range.contains(value)
            }
            _ => false, // if the provided Element of a different type than the given range type, contains returns false
        }
    }

    /// Provides optional non negative integer range
    pub fn optional() -> Self {
        Range::NonNegativeInteger(
            RangeImpl::range(
                RangeBoundaryValue::Value(0usize, RangeBoundaryType::Inclusive),
                RangeBoundaryValue::Value(1usize, RangeBoundaryType::Inclusive),
            )
            .unwrap(),
        )
    }

    /// Provides required non negative integer range
    pub fn required() -> Self {
        Range::NonNegativeInteger(
            RangeImpl::range(
                RangeBoundaryValue::Value(1usize, RangeBoundaryType::Inclusive),
                RangeBoundaryValue::Value(1usize, RangeBoundaryType::Inclusive),
            )
            .unwrap(),
        )
    }

    /// Parse an [Element] into a [Range] using the [RangeType]
    // `range_type` is used to determine range type for integer non negative ranges or number ranges
    pub fn from_ion_element(
        value: &Element,
        range_type: RangeType,
        isl_version: IslVersion,
    ) -> IonSchemaResult<Range> {
        // if an integer value is passed here then convert it into a range
        // eg. if `1` is passed as value then return a range [1,1]
        return if let Some(integer_value) = value.as_int() {
            match range_type {
                RangeType::Precision | RangeType::NonNegativeInteger => {
                    let non_negative_integer_value =
                        Range::validate_non_negative_integer_range_boundary_value(
                            value.as_int().unwrap(),
                            &range_type,
                        )?;
                    Ok(Range::NonNegativeInteger(non_negative_integer_value.into()))
                }
                RangeType::TimestampPrecision => invalid_schema_error(format!(
                    "Timestamp precision ranges can not be constructed from value of type {}",
                    value.ion_type()
                )),
                RangeType::Any => Ok(Range::Integer(integer_value.to_owned().into())),
                RangeType::NumberOrTimestamp => Ok(NumberRange::new(
                    RangeBoundaryValue::Value(integer_value.into(), RangeBoundaryType::Inclusive),
                    RangeBoundaryValue::Value(integer_value.into(), RangeBoundaryType::Inclusive),
                )?
                .into()),
            }
        } else if let Some(timestamp_precision_symbol) = value.as_symbol() {
            let timestamp_precision = timestamp_precision_symbol.text().ok_or_else(|| {
                invalid_schema_error_raw(
                    "Range can not be constructed from symbol with unknown text",
                )
            })?;
            if range_type == RangeType::TimestampPrecision {
                Ok(TimestampPrecisionRange::new(
                    RangeBoundaryValue::Value(
                        timestamp_precision.try_into()?,
                        RangeBoundaryType::Inclusive,
                    ),
                    RangeBoundaryValue::Value(
                        timestamp_precision.try_into()?,
                        RangeBoundaryType::Inclusive,
                    ),
                )?
                .into())
            } else {
                invalid_schema_error(format!(
                    "{:?} ranges can not be constructed from value of type {}",
                    range_type,
                    value.ion_type()
                ))
            }
        } else if let element::Value::List(range) = value.value() {
            // verify if the value has annotation range
            if !value.annotations().contains("range") {
                return invalid_schema_error(
                    "An element representing a range must have the annotation `range`.",
                );
            }

            // verify that the range sequence has only two values i.e. start and end range boundary values
            if range.len() != 2 {
                return invalid_schema_error(
                    "Ranges must contain two values representing the minimum and maximum ends of range.",
                );
            }

            let start = try_to!(range.get(0));
            let end = try_to!(range.get(1));

            // this match statement determines that no range types other then the below range types are allowed
            match start.ion_type() {
                IonType::Symbol
                | IonType::Int
                | IonType::Float
                | IonType::Decimal
                | IonType::Timestamp => Ok(Self::validate_and_construct_range(
                    TypedRangeBoundaryValue::from_ion_element(
                        start,
                        range_type.to_owned(),
                        isl_version,
                    )?,
                    TypedRangeBoundaryValue::from_ion_element(end, range_type, isl_version)?,
                )?),
                _ => invalid_schema_error("Unsupported range type specified"),
            }
        } else {
            invalid_schema_error(format!(
                "Ranges can not be constructed for type {}",
                value.ion_type()
            ))
        };
    }

    // helper method to which validates a non negative integer range boundary value
    pub(crate) fn validate_non_negative_integer_range_boundary_value(
        value: &Int,
        range_type: &RangeType,
    ) -> IonSchemaResult<usize> {
        // minimum precision must be greater than or equal to 1
        // for more information: https://amazon-ion.github.io/ion-schema/docs/isl-1-0/spec#precision
        let min_value = i64::from(range_type == &RangeType::Precision);
        match value.as_i64() {
            Some(v) => {
                if v >= min_value {
                    match v.try_into() {
                        Err(_) => invalid_schema_error(format!(
                            "Expected non negative integer greater than {min_value} for range boundary values, found {v}"
                        )),
                        Ok(non_negative_int_value) => Ok(non_negative_int_value),
                    }
                } else {
                    invalid_schema_error(format!(
                        "Expected non negative integer greater than {min_value} for range boundary values, found {v}"
                    ))
                }
            }
            None => match value.as_big_int() {
                None => {
                    unreachable!("Expected range boundary values must be a non negative integer")
                }
                Some(v) => {
                    if v >= &BigInt::from(min_value) {
                        match v.try_into() {
                            Err(_) => invalid_schema_error(format!(
                                "Expected non negative integer greater than {min_value} for range boundary values, found {v}"
                            )),
                            Ok(non_negative_int_value) => Ok(non_negative_int_value),
                        }
                    } else {
                        invalid_schema_error(format!(
                            "Expected non negative integer greater than {min_value} for range boundary values, found {v}"
                        ))
                    }
                }
            },
        }
    }

    // helper method to validate range boundary values and construct a `Range`
    pub(crate) fn validate_and_construct_range(
        start: TypedRangeBoundaryValue,
        end: TypedRangeBoundaryValue,
    ) -> IonSchemaResult<Range> {
        // validate the range boundary values : `start` and `end`
        match (&start, &end) {
            (TypedRangeBoundaryValue::Min, TypedRangeBoundaryValue::Max) => {
                invalid_schema_error("Range boundaries can not be min and max together (i.e. range::[min, max] is not allowed)")
            }
            (TypedRangeBoundaryValue::Max, _) => {
                invalid_schema_error("Lower range boundary value must not be max")
            }
            (_, TypedRangeBoundaryValue::Min) => {
                invalid_schema_error("Upper range boundary value must not be min")
            }
            (_, TypedRangeBoundaryValue::Integer(_)) | (TypedRangeBoundaryValue::Integer(_), _) => {
                Ok(Range::Integer(RangeImpl::<Int>::int_range_from_typed_boundary_value(start, end)?))
            }
            (_, TypedRangeBoundaryValue::NonNegativeInteger(_)) |
            (TypedRangeBoundaryValue::NonNegativeInteger(_), _) => {
                Ok(Range::NonNegativeInteger(RangeImpl::<usize>::int_non_negative_range_from_typed_boundary_value(start, end)?))
            }
            (_, TypedRangeBoundaryValue::TimestampPrecision(_)) |
            (TypedRangeBoundaryValue::TimestampPrecision(_), _) => {
                Ok(Range::TimestampPrecision(RangeImpl::<TimestampPrecision>::timestamp_precision_range_from_typed_boundary_value(start, end)?))
            }
            (_, TypedRangeBoundaryValue::Float(_)) |
            (TypedRangeBoundaryValue::Float(_), _) => {
                Ok(Range::Float(RangeImpl::<f64>::float_range_from_typed_boundary_value(start, end)?))
            }
            (_, TypedRangeBoundaryValue::Decimal(_)) |
            (TypedRangeBoundaryValue::Decimal(_), _) => {
                Ok(Range::Decimal(RangeImpl::<Decimal>::decimal_range_from_typed_boundary_value(start, end)?))
            }
            (_, TypedRangeBoundaryValue::Number(_)) |
            (TypedRangeBoundaryValue::Number(_), _) => {
                Ok(Range::Number(RangeImpl::<Number>::number_range_from_typed_boundary_value(start, end)?))
            }
            (_, TypedRangeBoundaryValue::Timestamp(_)) |
            (TypedRangeBoundaryValue::Timestamp(_), _) => {
                Ok(Range::Timestamp(RangeImpl::<Timestamp>::timestamp_range_from_typed_boundary_value(start, end)?))
            }
        }
    }
}

impl From<IntegerRange> for Range {
    fn from(value: IntegerRange) -> Self {
        Range::Integer(value)
    }
}

impl From<NonNegativeIntegerRange> for Range {
    fn from(value: NonNegativeIntegerRange) -> Self {
        Range::NonNegativeInteger(value)
    }
}

impl From<FloatRange> for Range {
    fn from(value: FloatRange) -> Self {
        Range::Float(value)
    }
}

impl From<DecimalRange> for Range {
    fn from(value: DecimalRange) -> Self {
        Range::Decimal(value)
    }
}

impl From<TimestampRange> for Range {
    fn from(value: TimestampRange) -> Self {
        Range::Timestamp(value)
    }
}

impl From<TimestampPrecisionRange> for Range {
    fn from(value: TimestampPrecisionRange) -> Self {
        Range::TimestampPrecision(value)
    }
}

impl From<NumberRange> for Range {
    fn from(value: NumberRange) -> Self {
        Range::Number(value)
    }
}

impl Display for Range {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self {
            Range::Integer(integer) => write!(f, "{integer}"),
            Range::NonNegativeInteger(non_negative_integer) => {
                write!(f, "{non_negative_integer}")
            }
            Range::TimestampPrecision(timestamp_precision) => write!(f, "{timestamp_precision}"),
            Range::Timestamp(timestamp) => write!(f, "{timestamp}"),
            Range::Decimal(decimal) => write!(f, "{decimal}"),
            Range::Float(float) => write!(f, "{float}"),
            Range::Number(number) => write!(f, "{number}"),
        }
    }
}

impl WriteToIsl for Range {
    fn write_to<W: IonWriter>(&self, writer: &mut W) -> IonSchemaResult<()> {
        writer.set_annotations(["range"]);
        match &self {
            Range::Integer(integer) => integer.write_to(writer)?,
            Range::NonNegativeInteger(non_negative_integer) => {
                non_negative_integer.write_to(writer)?
            }
            Range::TimestampPrecision(timestamp_precision) => {
                timestamp_precision.write_to(writer)?
            }
            Range::Timestamp(timestamp) => timestamp.write_to(writer)?,
            Range::Decimal(decimal) => decimal.write_to(writer)?,
            Range::Float(float) => float.write_to(writer)?,
            Range::Number(number) => number.write_to(writer)?,
        }
        Ok(())
    }
}

/// Represents a generic range where some constraints can be defined using this range
// this is a generic implementation of ranges
#[derive(Debug, Clone, PartialEq)]
pub struct RangeImpl<T> {
    start: RangeBoundaryValue<T>,
    end: RangeBoundaryValue<T>,
}

impl<T: std::cmp::PartialOrd> RangeImpl<T> {
    /// Provides a way to generate generic range using the start and end values
    pub fn range(
        start: RangeBoundaryValue<T>,
        end: RangeBoundaryValue<T>,
    ) -> IonSchemaResult<Self> {
        if start == end
            && (start.range_boundary_type() == &RangeBoundaryType::Exclusive
                || end.range_boundary_type() == &RangeBoundaryType::Exclusive)
        {
            return invalid_schema_error("Empty ranges are not allowed");
        }
        if start > end {
            return invalid_schema_error(
                "Lower range boundary value can not be bigger than upper range boundary",
            );
        }
        Ok(RangeImpl { start, end })
    }

    /// Provides a boolean value to specify whether the given value is within the range or not
    pub fn contains(&self, value: T) -> bool {
        let is_in_lower_bound = match &self.start {
            Min => true,
            Value(start_value, boundary_type) => match boundary_type {
                RangeBoundaryType::Inclusive => start_value <= &value,
                RangeBoundaryType::Exclusive => start_value < &value,
            },
            Max => unreachable!("Cannot have 'Max' as the lower range boundary"),
        };

        let is_in_upper_bound = match &self.end {
            Max => true,
            Min => unreachable!("Cannot have 'Min' as the upper range boundary"),
            Value(end_value, boundary_type) => match boundary_type {
                RangeBoundaryType::Inclusive => end_value >= &value,
                RangeBoundaryType::Exclusive => end_value > &value,
            },
        };
        is_in_upper_bound && is_in_lower_bound
    }

    /// Provides `RangeImpl<Integer>` for given `TypedRangeBoundaryValue`
    // this method requires a prior check for TypedRangeBoundaryValue::Integer
    pub(crate) fn int_range_from_typed_boundary_value(
        start: TypedRangeBoundaryValue,
        end: TypedRangeBoundaryValue,
    ) -> IonSchemaResult<RangeImpl<Int>> {
        match (start, end) {
            (
                TypedRangeBoundaryValue::Integer(RangeBoundaryValue::Value(v1, v1_type)),
                TypedRangeBoundaryValue::Integer(RangeBoundaryValue::Value(v2, v2_type)),
            ) => {
                // Safe unwrap since as_big_int returns None for i64 value
                let v1_as_big_int = v1
                    .as_big_int()
                    .map(|v| v.to_owned())
                    .unwrap_or(BigInt::from(v1.as_i64().unwrap()));

                let v2_as_big_int = v2
                    .as_big_int()
                    .map(|v| v.to_owned())
                    .unwrap_or(BigInt::from(v2.as_i64().unwrap()));

                // verify this is not an empty range for which there is no valid integer values
                if (v2_as_big_int - v1_as_big_int).is_one()
                    && v1_type == RangeBoundaryType::Exclusive
                    && v2_type == RangeBoundaryType::Exclusive
                {
                    return invalid_schema_error("No valid values in the Integer range");
                }
                RangeImpl::range(
                    RangeBoundaryValue::Value(v1, v1_type),
                    RangeBoundaryValue::Value(v2, v2_type),
                )
            }
            (TypedRangeBoundaryValue::Min, TypedRangeBoundaryValue::Integer(v2)) => {
                RangeImpl::range(RangeBoundaryValue::Min, v2)
            }
            (TypedRangeBoundaryValue::Integer(v1), TypedRangeBoundaryValue::Max) => {
                RangeImpl::range(v1, RangeBoundaryValue::Max)
            }
            (TypedRangeBoundaryValue::Integer(Value(v1, _)), _)
            | (_, TypedRangeBoundaryValue::Integer(Value(v1, _))) => {
                invalid_schema_error("Range boundaries must have the same types")
            }
            _ => unreachable!(
                "Integer ranges can not be constructed with non integer range boundary types"
            ),
        }
    }

    /// Provides `RangeImpl<usize>` for given `TypedRangeBoundaryValue`
    // this method requires a prior check for TypedRangeBoundaryValue::NonNegativeInteger
    pub(crate) fn int_non_negative_range_from_typed_boundary_value(
        start: TypedRangeBoundaryValue,
        end: TypedRangeBoundaryValue,
    ) -> IonSchemaResult<RangeImpl<usize>> {
        match (start, end) {
            (TypedRangeBoundaryValue::NonNegativeInteger(Value(v1, v1_type)), TypedRangeBoundaryValue::NonNegativeInteger(Value(v2, v2_type))) => {
                // verify this is not an empty range (i.e. one for which there are no valid non-negative integer values)
                if v2 > v1 && v2 - v1 == 1
                    && v1_type == RangeBoundaryType::Exclusive
                    && v2_type == RangeBoundaryType::Exclusive
                {
                    return invalid_schema_error("No valid values in the Integer range");
                }
                RangeImpl::range(
                    Value(v1, v1_type),
                    Value(v2, v2_type),
                )
            }
            (TypedRangeBoundaryValue::Min, TypedRangeBoundaryValue::NonNegativeInteger(v2)) => {
                RangeImpl::range(RangeBoundaryValue::Min, v2)
            }
            (TypedRangeBoundaryValue::NonNegativeInteger(v1), TypedRangeBoundaryValue::Max) => {
                RangeImpl::range(v1, RangeBoundaryValue::Max)
            }
            (TypedRangeBoundaryValue::NonNegativeInteger(Value(v1, _)), _) | (_, TypedRangeBoundaryValue::NonNegativeInteger(Value(v1, _)))=> {
                invalid_schema_error("Range boundaries should have same types")
            }
            _ => unreachable!(
                "NonNegativeInteger ranges can not be constructed with non integer non negative range boundary types"
            ),
        }
    }

    /// Provides `RangeImpl<Number>` for given `TypedRangeBoundaryValue`
    // this method requires a prior check for TypedRangeBoundaryValue::Number
    pub(crate) fn number_range_from_typed_boundary_value(
        start: TypedRangeBoundaryValue,
        end: TypedRangeBoundaryValue,
    ) -> IonSchemaResult<RangeImpl<Number>> {
        match (start, end) {
            (TypedRangeBoundaryValue::Number(v1), TypedRangeBoundaryValue::Number(v2)) => {
                RangeImpl::range(v1, v2)
            }
            (TypedRangeBoundaryValue::Min, TypedRangeBoundaryValue::Number(v2)) => {
                RangeImpl::range(RangeBoundaryValue::Min, v2)
            }
            (TypedRangeBoundaryValue::Number(v1), TypedRangeBoundaryValue::Max) => {
                RangeImpl::range(v1, RangeBoundaryValue::Max)
            }
            (TypedRangeBoundaryValue::Number(Value(v1, _)), _)
            | (_, TypedRangeBoundaryValue::Number(Value(v1, _))) => {
                invalid_schema_error("Range boundaries should have same types")
            }
            _ => unreachable!(
                "Number ranges can not be constructed with non number range boundary types"
            ),
        }
    }

    /// Provides `RangeImpl<TimestampPrecision>` for given `TypedRangeBoundaryValue`
    // this method requires a prior check for TypedRangeBoundaryValue::TimestampPrecision
    pub(crate) fn timestamp_precision_range_from_typed_boundary_value(
        start: TypedRangeBoundaryValue,
        end: TypedRangeBoundaryValue,
    ) -> IonSchemaResult<RangeImpl<TimestampPrecision>> {
        match (start, end) {
            (TypedRangeBoundaryValue::TimestampPrecision(v1), TypedRangeBoundaryValue::TimestampPrecision(v2)) => {
                RangeImpl::range(v1, v2)
            }
            (TypedRangeBoundaryValue::Min, TypedRangeBoundaryValue::TimestampPrecision(v2)) => {
                RangeImpl::range(RangeBoundaryValue::Min, v2)
            }
            (TypedRangeBoundaryValue::TimestampPrecision(v1), TypedRangeBoundaryValue::Max) => {
                RangeImpl::range(v1, RangeBoundaryValue::Max)
            }
            (TypedRangeBoundaryValue::TimestampPrecision(Value(v1, _)), _) | (_, TypedRangeBoundaryValue::TimestampPrecision(Value(v1, _)))=> {
                invalid_schema_error("Range boundaries should have same types")
            }
            _ => unreachable!(
                "TimestampPrecision ranges can not be constructed with non timestamp precision range boundary types"
            ),
        }
    }

    /// Provides `RangeImpl<Decimal>` for given `TypedRangeBoundaryValue`
    // this method requires a prior check for TypedRangeBoundaryValue::Decimal
    pub(crate) fn decimal_range_from_typed_boundary_value(
        start: TypedRangeBoundaryValue,
        end: TypedRangeBoundaryValue,
    ) -> IonSchemaResult<RangeImpl<Decimal>> {
        match (start, end) {
            (TypedRangeBoundaryValue::Decimal(v1), TypedRangeBoundaryValue::Decimal(v2)) => {
                RangeImpl::range(v1, v2)
            }
            (TypedRangeBoundaryValue::Min, TypedRangeBoundaryValue::Decimal(v2)) => {
                RangeImpl::range(RangeBoundaryValue::Min, v2)
            }
            (TypedRangeBoundaryValue::Decimal(v1), TypedRangeBoundaryValue::Max) => {
                RangeImpl::range(v1, RangeBoundaryValue::Max)
            }
            (TypedRangeBoundaryValue::Decimal(Value(v1, _)), _)
            | (_, TypedRangeBoundaryValue::Decimal(Value(v1, _))) => {
                invalid_schema_error("Range boundaries should have same types")
            }
            _ => unreachable!(
                "Decimal ranges can not be constructed with non decimal range boundary types"
            ),
        }
    }

    /// Provides `RangeImpl<f64>` for given `TypedRangeBoundaryValue`
    // this method requires a prior check for TypedRangeBoundaryValue::Float
    pub(crate) fn float_range_from_typed_boundary_value(
        start: TypedRangeBoundaryValue,
        end: TypedRangeBoundaryValue,
    ) -> IonSchemaResult<RangeImpl<f64>> {
        match (start, end) {
            (TypedRangeBoundaryValue::Float(v1), TypedRangeBoundaryValue::Float(v2)) => {
                RangeImpl::range(v1, v2)
            }
            (TypedRangeBoundaryValue::Min, TypedRangeBoundaryValue::Float(v2)) => {
                RangeImpl::range(RangeBoundaryValue::Min, v2)
            }
            (TypedRangeBoundaryValue::Float(v1), TypedRangeBoundaryValue::Max) => {
                RangeImpl::range(v1, RangeBoundaryValue::Max)
            }
            (TypedRangeBoundaryValue::Float(Value(v1, _)), _)
            | (_, TypedRangeBoundaryValue::Float(Value(v1, _))) => {
                invalid_schema_error("Range boundaries should have same types")
            }
            _ => unreachable!(
                "Float ranges can not be constructed with non float range boundary types"
            ),
        }
    }

    /// Provides `RangeImpl<Timestamp>` for given `TypedRangeBoundaryValue`
    // this method requires a prior check for TypedRangeBoundaryValue::Timestamp
    pub(crate) fn timestamp_range_from_typed_boundary_value(
        start: TypedRangeBoundaryValue,
        end: TypedRangeBoundaryValue,
    ) -> IonSchemaResult<RangeImpl<Timestamp>> {
        match (start, end) {
            (TypedRangeBoundaryValue::Timestamp(v1), TypedRangeBoundaryValue::Timestamp(v2)) => {
                RangeImpl::range(v1, v2)
            }
            (TypedRangeBoundaryValue::Min, TypedRangeBoundaryValue::Timestamp(v2)) => {
                RangeImpl::range(RangeBoundaryValue::Min, v2)
            }
            (TypedRangeBoundaryValue::Timestamp(v1), TypedRangeBoundaryValue::Max) => {
                RangeImpl::range(v1, RangeBoundaryValue::Max)
            }
            (TypedRangeBoundaryValue::Timestamp(Value(v1, _)), _)
            | (_, TypedRangeBoundaryValue::Timestamp(Value(v1, _))) => {
                invalid_schema_error("Range boundaries should have same types")
            }
            _ => unreachable!(
                "Timestamp ranges can not be constructed with non timestamp range boundary types"
            ),
        }
    }
}

/// Provides `Range` for given `usize`
impl From<usize> for RangeImpl<usize> {
    fn from(non_negative_int_value: usize) -> Self {
        RangeImpl::range(
            RangeBoundaryValue::Value(non_negative_int_value, RangeBoundaryType::Inclusive),
            RangeBoundaryValue::Value(non_negative_int_value, RangeBoundaryType::Inclusive),
        )
        .unwrap()
    }
}

/// Provides `Range` for given `Integer`
impl From<Int> for RangeImpl<Int> {
    fn from(int_value: Int) -> Self {
        RangeImpl::range(
            RangeBoundaryValue::Value(int_value.to_owned(), RangeBoundaryType::Inclusive),
            RangeBoundaryValue::Value(int_value, RangeBoundaryType::Inclusive),
        )
        .unwrap()
    }
}

/// Provides `Range` for given `&str`
impl TryFrom<&str> for RangeImpl<TimestampPrecision> {
    type Error = IonSchemaError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let timestamp_precision: TimestampPrecision = value.try_into()?;
        RangeImpl::range(
            RangeBoundaryValue::Value(timestamp_precision.to_owned(), RangeBoundaryType::Inclusive),
            RangeBoundaryValue::Value(timestamp_precision, RangeBoundaryType::Inclusive),
        )
    }
}

impl<T: PartialOrd> RangeImpl<T> {
    pub fn new<S, E>(start: S, end: E) -> IonSchemaResult<Self>
    where
        S: Into<RangeBoundaryValue<T>>,
        E: Into<RangeBoundaryValue<T>>,
    {
        let start = start.into();
        let end = end.into();
        RangeImpl::range(start, end)
    }
}

impl<T: Display> Display for RangeImpl<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "range::[ {}, {} ]", &self.start, &self.end)
    }
}

impl<T: Display> WriteToIsl for RangeImpl<T> {
    fn write_to<W: IonWriter>(&self, writer: &mut W) -> IonSchemaResult<()> {
        writer.step_in(IonType::List)?;
        self.start.write_to(writer)?;
        self.end.write_to(writer)?;
        writer.step_out()?;
        Ok(())
    }
}

// This lets us turn any `T` into a RangeBoundaryValue<T>::Value(_, Inclusive)
impl<T> From<T> for RangeBoundaryValue<T> {
    fn from(value: T) -> RangeBoundaryValue<T> {
        RangeBoundaryValue::Value(value, RangeBoundaryType::Inclusive)
    }
}

/// Provides typed range boundary values
// this is a wrapper around generic `RangeBoundaryValue` and is used when generating ranges from an ion IonElement
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub(crate) enum TypedRangeBoundaryValue {
    Min,
    Max,
    Integer(RangeBoundaryValue<Int>),
    NonNegativeInteger(RangeBoundaryValue<usize>),
    TimestampPrecision(RangeBoundaryValue<TimestampPrecision>),
    Float(RangeBoundaryValue<f64>),
    Decimal(RangeBoundaryValue<Decimal>),
    Number(RangeBoundaryValue<Number>),
    Timestamp(RangeBoundaryValue<Timestamp>),
}

impl TypedRangeBoundaryValue {
    fn from_ion_element(
        boundary: &Element,
        range_type: RangeType,
        isl_version: IslVersion,
    ) -> IonSchemaResult<TypedRangeBoundaryValue> {
        let range_boundary_type = if boundary.annotations().contains("exclusive") {
            RangeBoundaryType::Exclusive
        } else {
            RangeBoundaryType::Inclusive
        };
        match boundary.ion_type() {
            IonType::Symbol => {
                let sym = try_to!(try_to!(boundary.as_symbol()).text());

                if (sym == "min" || sym == "max")
                    && range_boundary_type == RangeBoundaryType::Exclusive
                {
                    return invalid_schema_error(
                        "Exclusive min or max are not allowed for range boundary values",
                    );
                }

                match sym {
                    "min" => Ok(TypedRangeBoundaryValue::Min),
                    "max" => Ok(TypedRangeBoundaryValue::Max),
                    _ => Ok(TypedRangeBoundaryValue::TimestampPrecision(
                        RangeBoundaryValue::Value(sym.try_into()?, range_boundary_type),
                    )),
                }
            }
            IonType::Int => {
                return match range_type {
                     RangeType::Precision | RangeType::NonNegativeInteger => {
                         Ok(TypedRangeBoundaryValue::NonNegativeInteger(
                             RangeBoundaryValue::Value(
                                 Range::validate_non_negative_integer_range_boundary_value(
                                     boundary.as_int().unwrap(),
                                     &range_type,
                                 )?,
                                 range_boundary_type,
                             ),
                         ))
                     },
                     RangeType::Any => {
                         Ok(TypedRangeBoundaryValue::Integer(RangeBoundaryValue::Value(
                             boundary.as_int().unwrap().to_owned(),
                             range_boundary_type,
                         )))
                     },
                     RangeType::TimestampPrecision => invalid_schema_error(
                    "Timestamp precision ranges can not be constructed for integer boundary values",
                ),
                RangeType::NumberOrTimestamp => Ok(TypedRangeBoundaryValue::Number(RangeBoundaryValue::Value(
                    boundary.as_int().unwrap().into(),
                    range_boundary_type,
                ))),
                 };
            }
            IonType::Decimal => match range_type {
                RangeType::NumberOrTimestamp => {
                    Ok(TypedRangeBoundaryValue::Number(RangeBoundaryValue::Value(
                        boundary.as_decimal().unwrap().into(),
                        range_boundary_type,
                    )))
                }
                RangeType::Any => Ok(TypedRangeBoundaryValue::Decimal(RangeBoundaryValue::Value(
                    boundary.as_decimal().unwrap().to_owned(),
                    range_boundary_type,
                ))),
                _ => invalid_schema_error(format!(
                    "{range_type:?} ranges can not be constructed for decimal boundary values"
                )),
            },
            IonType::Float => match range_type {
                RangeType::NumberOrTimestamp => {
                    Ok(TypedRangeBoundaryValue::Number(RangeBoundaryValue::Value(
                        boundary.as_float().unwrap().to_owned().try_into()?,
                        range_boundary_type,
                    )))
                }
                RangeType::Any => Ok(TypedRangeBoundaryValue::Float(RangeBoundaryValue::Value(
                    boundary.as_float().unwrap().to_owned(),
                    range_boundary_type,
                ))),
                _ => invalid_schema_error(format!(
                    "{range_type:?} ranges can not be constructed for float boundary values"
                )),
            },
            IonType::Timestamp => match range_type {
                RangeType::NumberOrTimestamp | RangeType::Any => {
                    // For ISL 1.0, verify that range boundary here doesn't have an unknown offset
                    // For timestamp ranges neither boundaries should have an unknown offset
                    if isl_version == IslVersion::V1_0
                        && boundary.as_timestamp().unwrap().offset().is_none()
                    {
                        return invalid_schema_error(
                            "Timestamp range boundary can not have an unknown offset",
                        );
                    }
                    Ok(TypedRangeBoundaryValue::Timestamp(
                        RangeBoundaryValue::Value(
                            boundary.as_timestamp().unwrap().to_owned(),
                            range_boundary_type,
                        ),
                    ))
                }
                _ => invalid_schema_error(format!(
                    "{range_type:?} ranges can not be constructed for timestamp boundary values"
                )),
            },
            _ => invalid_schema_error(format!(
                "Unsupported range boundary type specified {}",
                boundary.ion_type()
            )),
        }
    }
}
/// Represents a range boundary value (i.e. min, max or a value in terms of [RangeBoundaryType])
#[derive(Debug, Clone)]
pub enum RangeBoundaryValue<T> {
    Max,
    Min,
    Value(T, RangeBoundaryType),
}

impl<T> RangeBoundaryValue<T> {
    pub fn range_boundary_type(&self) -> &RangeBoundaryType {
        match self {
            Value(_, range_boundary_type) => range_boundary_type,
            _ => &RangeBoundaryType::Inclusive,
        }
    }

    pub fn range_boundary_value(&self) -> Option<&T> {
        match self {
            Value(v, _) => Some(v),
            _ => None,
        }
    }
}

// This PartialEq implementation doesn't consider RangeBoundaryType for equivalence
impl<T: std::cmp::PartialEq> PartialEq for RangeBoundaryValue<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self, other) {
            (Max, Max) => true,
            (Max, _) => false,
            (Min, Min) => true,
            (Min, _) => false,
            (Value(v1, _), Value(v2, _)) => v1 == v2,
            (Value(_, _), _) => false,
        }
    }
}

impl<T: std::cmp::PartialOrd> PartialOrd for RangeBoundaryValue<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (&self, other) {
            (Max, _) => Some(Ordering::Greater),
            (_, Min) => Some(Ordering::Greater),
            (_, Max) => Some(Ordering::Less),
            (Min, _) => Some(Ordering::Less),
            (Value(v1, this_range_boundary_type), Value(v2, that_range_boundary_type)) => {
                v1.partial_cmp(v2)
            }
        }
    }
}

impl<T: Display> Display for RangeBoundaryValue<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            match &self {
                Max => "max".to_string(),
                Min => "min".to_string(),
                Value(value, range_boundary_type) => {
                    format!("{range_boundary_type}{value}")
                }
            }
        )
    }
}

impl<T: Display> WriteToIsl for RangeBoundaryValue<T> {
    fn write_to<W: IonWriter>(&self, writer: &mut W) -> IonSchemaResult<()> {
        match &self {
            Max => writer.write_symbol("max")?,
            Min => writer.write_symbol("min")?,
            Value(value, range_boundary_type) => {
                if range_boundary_type == &RangeBoundaryType::Exclusive {
                    writer.set_annotations(["exclusive"]);
                }
                let element = Element::read_one(format!("{value}").as_bytes())?;
                writer.write_element(&element)?;
            }
        }
        Ok(())
    }
}

/// Represents the range boundary types in terms of exclusivity (i.e. inclusive or exclusive)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum RangeBoundaryType {
    Inclusive,
    Exclusive,
}

impl Display for RangeBoundaryType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            match &self {
                RangeBoundaryType::Inclusive => "",
                RangeBoundaryType::Exclusive => "exclusive::",
            }
        )
    }
}

/// Represents if the range is non negative integer range or not
/// This will be used while creating an integer range from Element
/// to explicitly state if its non negative or not
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeType {
    Precision, // used by precision constraint to specify non negative integer precision with minimum value as `1`
    NonNegativeInteger, // used by byte_length, container_length and codepoint_length to specify non negative integer range
    TimestampPrecision, // used by timestamp_precision to specify timestamp precision range
    NumberOrTimestamp,  // used by valid_values constraint
    Any,                // used for any range types (e.g. Integer, Float, Timestamp, Decimal)
}

/// Represents number boundary values
/// A number can be float, integer or decimal
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Number {
    big_decimal_value: BigDecimal,
}

impl Number {
    pub fn new(big_decimal_value: BigDecimal) -> Self {
        Self { big_decimal_value }
    }

    pub fn big_decimal_value(&self) -> &BigDecimal {
        &self.big_decimal_value
    }
}

impl TryFrom<f64> for Number {
    type Error = IonSchemaError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        // Note: could not use BigDecimal's `try_from` method here as that uses `DIGITS` instead of `MANTISSA_DIGITS`.
        // `DIGITS` gives an approximate number of significant digits, which failed a test from ion-schema-tests test suite
        Ok(Number {
            big_decimal_value: BigDecimal::from_str(&format!(
                "{:.PRECISION$e}",
                value,
                PRECISION = f64::MANTISSA_DIGITS as usize
            ))
            .map_err(|err| {
                invalid_schema_error_raw(format!("Cannot convert f64 to BigDecimal for {value}"))
            })?,
        })
    }
}

impl From<&Decimal> for Number {
    fn from(value: &Decimal) -> Self {
        let mut value = value.to_owned();
        // When Decimal is converted to BigDecimal, it returns an Error if the Decimal being
        // converted is a negative zero, which BigDecimal cannot represent. Otherwise returns Ok.
        // hence if we detect negative zero we convert it to zero and make this infallible
        if value.is_zero() {
            value = Decimal::from(0);
        }
        Number {
            big_decimal_value: value.try_into().unwrap(),
        }
    }
}

impl From<&Int> for Number {
    fn from(value: &Int) -> Self {
        Number {
            big_decimal_value: match value {
                Int::I64(int_val) => int_val.to_owned().into(),
                Int::BigInt(big_int_val) => big_int_val.to_owned().into(),
            },
        }
    }
}

impl Display for Number {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", &self.big_decimal_value)
    }
}
