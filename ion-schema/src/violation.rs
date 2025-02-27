use crate::ion_path::IonPath;
use std::fmt;
use std::fmt::Formatter;
use thiserror::Error;

/// Represents [Violation] found during validation with detailed error message, error code and the constraint for which the validation failed
#[derive(Debug, Clone, PartialEq, Error)]
pub struct Violation {
    constraint: String,  // represents the constraint that created this violation
    code: ViolationCode, // represents an error code that indicates the type of the violation
    message: String,     // represents the detailed error message for this violation
    ion_path: IonPath,   // represents the path to Ion value for which violation occurred
    violations: Vec<Violation>,
}

impl Violation {
    pub fn new<A: AsRef<str>, B: AsRef<str>>(
        constraint: A,
        code: ViolationCode,
        message: B,
        ion_path: &mut IonPath,
    ) -> Self {
        Self {
            constraint: constraint.as_ref().to_owned(),
            code,
            message: message.as_ref().to_owned(),
            ion_path: ion_path.to_owned(),
            violations: vec![],
        }
    }

    pub fn with_violations<A: AsRef<str>, B: AsRef<str>>(
        constraint: A,
        code: ViolationCode,
        message: B,
        ion_path: &mut IonPath,
        violations: Vec<Violation>,
    ) -> Self {
        Self {
            constraint: constraint.as_ref().to_owned(),
            code,
            message: message.as_ref().to_owned(),
            ion_path: ion_path.to_owned(),
            violations,
        }
    }

    pub fn ion_path(&self) -> &IonPath {
        &self.ion_path
    }

    pub fn message(&self) -> &String {
        &self.message
    }

    pub fn code(&self) -> &ViolationCode {
        &self.code
    }

    /// Provides flattened list of leaf violations which represent the root cause of the top-level violation.
    pub fn flattened_violations(&self) -> Vec<&Violation> {
        let mut flattened_violations = Vec::new();
        self.flatten_violations(&mut flattened_violations);
        flattened_violations
    }

    fn flatten_violations<'a>(&'a self, flattened: &mut Vec<&'a Violation>) {
        if self.violations.is_empty() {
            flattened.push(self);
        }
        for violation in &self.violations {
            if violation.violations.is_empty() {
                flattened.push(violation);
            } else {
                violation.flatten_violations(flattened)
            }
        }
    }

    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }
}

// TODO: Implement Violation with proper indentation for the nested tree of violations
impl fmt::Display for Violation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "A validation error occurred: {}", self.message)
    }
}

/// Represents violation code that indicates the type of the violation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationCode {
    AllTypesNotMatched,
    AnnotationMismatched,
    ElementMismatched,     // this is used for mismatched elements in containers
    ElementNotDistinct,    // this is used for elements that are not distinct in containers
    FieldNamesMismatched,  // this is used for mismatched field names in a struct
    FieldNamesNotDistinct, // this is used for field names that are not distinct in a struct
    FieldsNotMatched,
    InvalidIeee754Float, // this is used for ieee754_float constraint
    InvalidLength, // this is used for any length related constraints (e.g. container_length, byte_length, codepoint_length)
    InvalidNull,   // if the value is a null for type references that doesn't allow null
    InvalidOpenContent, // if a container contains open content when `content: closed` is specified
    InvalidValue,  // this is used for valid_values constraint
    MissingAnnotation, // if the annotation is missing for annotations constraint
    MissingValue,  // if the ion value is missing for a particular constraint
    MoreThanOneTypeMatched,
    NoTypesMatched,
    RegexMismatched, // this is used for regex constraint
    TypeConstraintsUnsatisfied,
    TypeMatched,
    TypeMismatched,
    UnexpectedAnnotation, // if unexpected annotation is found for annotations constraint
}

impl fmt::Display for ViolationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ViolationCode::AllTypesNotMatched => "all_types_not_matched",
                ViolationCode::AnnotationMismatched => "annotation_mismatched",
                ViolationCode::ElementMismatched => "element_mismatched",
                ViolationCode::ElementNotDistinct => "element_not_distinct",
                ViolationCode::FieldNamesMismatched => "field_names_mismatched",
                ViolationCode::FieldNamesNotDistinct => "field_names_not_distinct",
                ViolationCode::FieldsNotMatched => "fields_not_matched",
                ViolationCode::InvalidIeee754Float => "invalid_ieee754_float",
                ViolationCode::InvalidLength => "invalid_length",
                ViolationCode::InvalidNull => "invalid_null",
                ViolationCode::InvalidOpenContent => "invalid_open_content",
                ViolationCode::InvalidValue => "invalid_value",
                ViolationCode::MissingAnnotation => "missing_annotation",
                ViolationCode::MissingValue => "missing_value",
                ViolationCode::MoreThanOneTypeMatched => "more_than_one_type_matched",
                ViolationCode::NoTypesMatched => "no_types_matched",
                ViolationCode::RegexMismatched => "regex_mismatched",
                ViolationCode::TypeConstraintsUnsatisfied => "type_constraints_unsatisfied",
                ViolationCode::TypeMatched => "type_matched",
                ViolationCode::TypeMismatched => "type_mismatched",
                ViolationCode::UnexpectedAnnotation => "unexpected_annotation",
            }
        )
    }
}
