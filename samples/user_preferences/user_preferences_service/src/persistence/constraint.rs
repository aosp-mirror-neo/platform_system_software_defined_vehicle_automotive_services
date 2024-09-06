// Copyright 2024 Google LLC

use serde::{Deserialize, Serialize};
use user_preferences_api::setting::{
    setting_and_constraints::Constraints as ConstraintsProto,
    EnumConstraints as EnumConstraintsProto, FloatConstraints as FloatConstraintsProto,
    Int32Constraints as Int32ConstraintsProto, Int64Constraints as Int64ConstraintsProto,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Constraint {
    Float(FloatConstraint),
    Int32(Int32Constraint),
    Int64(Int64Constraint),
    Enum(EnumConstraint),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Int32Constraint {
    pub min_value: Option<i32>,
    pub max_value: Option<i32>,
    pub step: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Int64Constraint {
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub step: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EnumConstraint {
    pub possible_values: ::std::vec::Vec<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FloatConstraint {
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
    pub step: Option<f32>,
}

impl From<ConstraintsProto> for Constraint {
    fn from(value: ConstraintsProto) -> Self {
        match value {
            ConstraintsProto::FloatConstraints(v) => Constraint::Float(FloatConstraint {
                min_value: v.min_value,
                max_value: v.max_value,
                step: v.step,
            }),
            ConstraintsProto::Int32Constraints(v) => Constraint::Int32(Int32Constraint {
                min_value: v.min_value,
                max_value: v.max_value,
                step: v.step,
            }),
            ConstraintsProto::Int64Constraints(v) => Constraint::Int64(Int64Constraint {
                min_value: v.min_value,
                max_value: v.max_value,
                step: v.step,
            }),
            ConstraintsProto::EnumConstraints(v) => {
                Constraint::Enum(EnumConstraint { possible_values: v.possible_values })
            }
            _ => todo!(),
        }
    }
}

impl From<Constraint> for ConstraintsProto {
    fn from(value: Constraint) -> Self {
        match value {
            Constraint::Float(v) => ConstraintsProto::FloatConstraints(FloatConstraintsProto {
                min_value: v.min_value,
                max_value: v.max_value,
                step: v.step,
                ..Default::default()
            }),
            Constraint::Int32(v) => ConstraintsProto::Int32Constraints(Int32ConstraintsProto {
                min_value: v.min_value,
                max_value: v.max_value,
                step: v.step,
                ..Default::default()
            }),
            Constraint::Int64(v) => ConstraintsProto::Int64Constraints(Int64ConstraintsProto {
                min_value: v.min_value,
                max_value: v.max_value,
                step: v.step,
                ..Default::default()
            }),
            Constraint::Enum(v) => ConstraintsProto::EnumConstraints(EnumConstraintsProto {
                possible_values: v.possible_values,
                ..Default::default()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use user_preferences_api::setting::{
        setting_and_constraints::Constraints as ConstraintsProto,
        EnumConstraints as EnumConstraintsProto, FloatConstraints as FloatConstraintsProto,
        Int32Constraints as Int32ConstraintsProto, Int64Constraints as Int64ConstraintsProto,
    };

    #[test]
    fn constraints_from_constraints_proto_int32() {
        let proto = ConstraintsProto::Int32Constraints(Int32ConstraintsProto {
            min_value: Some(10),
            max_value: Some(20),
            step: Some(2),
            ..Default::default()
        });

        let constraints: Constraint = proto.into();

        assert_eq!(
            constraints,
            Constraint::Int32(Int32Constraint {
                min_value: Some(10),
                max_value: Some(20),
                step: Some(2),
            })
        );
    }

    #[test]
    fn constraints_from_constraints_proto_int64() {
        let proto = ConstraintsProto::Int64Constraints(Int64ConstraintsProto {
            min_value: Some(10),
            max_value: Some(20),
            step: Some(2),
            ..Default::default()
        });

        let constraints: Constraint = proto.into();

        assert_eq!(
            constraints,
            Constraint::Int64(Int64Constraint {
                min_value: Some(10),
                max_value: Some(20),
                step: Some(2),
            })
        );
    }

    #[test]
    fn constraints_from_constraints_proto_float() {
        let proto = ConstraintsProto::FloatConstraints(FloatConstraintsProto {
            min_value: Some(10.0),
            max_value: Some(20.0),
            step: Some(2.0),
            ..Default::default()
        });

        let constraints: Constraint = proto.into();

        assert_eq!(
            constraints,
            Constraint::Float(FloatConstraint {
                min_value: Some(10.0),
                max_value: Some(20.0),
                step: Some(2.0),
            })
        );
    }

    #[test]
    fn constraints_from_constraints_proto_enum() {
        let proto = ConstraintsProto::EnumConstraints(EnumConstraintsProto {
            possible_values: vec![1, 2, 3],
            ..Default::default()
        });

        let constraints: Constraint = proto.into();

        assert_eq!(
            constraints,
            Constraint::Enum(EnumConstraint { possible_values: vec![1, 2, 3] })
        );
    }

    #[test]
    fn constraints_to_constraints_proto_int32() {
        let constraints = Constraint::Int32(Int32Constraint {
            min_value: Some(10),
            max_value: Some(20),
            step: Some(2),
        });

        let proto: ConstraintsProto = constraints.into();

        assert_eq!(
            proto,
            ConstraintsProto::Int32Constraints(Int32ConstraintsProto {
                min_value: Some(10),
                max_value: Some(20),
                step: Some(2),
                ..Default::default()
            })
        );
    }

    #[test]
    fn constraints_to_constraints_proto_int64() {
        let constraints = Constraint::Int64(Int64Constraint {
            min_value: Some(10),
            max_value: Some(20),
            step: Some(2),
        });

        let proto: ConstraintsProto = constraints.into();

        assert_eq!(
            proto,
            ConstraintsProto::Int64Constraints(Int64ConstraintsProto {
                min_value: Some(10),
                max_value: Some(20),
                step: Some(2),
                ..Default::default()
            })
        );
    }

    #[test]
    fn constraints_to_constraints_proto_float() {
        let constraints = Constraint::Float(FloatConstraint {
            min_value: Some(10.0),
            max_value: Some(20.0),
            step: Some(2.0),
        });

        let proto: ConstraintsProto = constraints.into();

        assert_eq!(
            proto,
            ConstraintsProto::FloatConstraints(FloatConstraintsProto {
                min_value: Some(10.0),
                max_value: Some(20.0),
                step: Some(2.0),
                ..Default::default()
            })
        );
    }

    #[test]
    fn constraints_to_constraints_proto_enum() {
        let constraints = Constraint::Enum(EnumConstraint { possible_values: vec![1, 2, 3] });

        let proto: ConstraintsProto = constraints.into();

        assert_eq!(
            proto,
            ConstraintsProto::EnumConstraints(EnumConstraintsProto {
                possible_values: vec![1, 2, 3],
                ..Default::default()
            })
        );
    }
}
