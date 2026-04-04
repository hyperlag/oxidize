//! IR top-level declarations.

use crate::{IrStmt, IrType, IrTypeParam};
use serde::{Deserialize, Serialize};

/// A top-level declaration inside an [`crate::IrModule`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrDecl {
    /// A class declaration.
    Class(IrClass),
    /// An interface declaration.
    Interface(IrInterface),
    /// An enum declaration.
    Enum(IrEnum),
}

/// Access visibility modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Protected,
    #[default]
    PackagePrivate,
    Private,
}

/// A class declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrClass {
    pub name: String,
    pub visibility: Visibility,
    pub is_abstract: bool,
    pub is_final: bool,
    /// Type parameters, e.g. `[IrTypeParam { name: "T", bounds: [] }]` for `class Box<T>`.
    pub type_params: Vec<IrTypeParam>,
    /// Superclass, if any (fully-qualified name).
    pub superclass: Option<String>,
    /// Implemented interfaces (fully-qualified names).
    pub interfaces: Vec<String>,
    pub fields: Vec<IrField>,
    pub methods: Vec<IrMethod>,
    pub constructors: Vec<IrConstructor>,
}

/// A field declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrField {
    pub name: String,
    pub ty: IrType,
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_final: bool,
    pub is_volatile: bool,
    pub init: Option<crate::IrExpr>,
}

/// A method declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrMethod {
    pub name: String,
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_synchronized: bool,
    pub type_params: Vec<IrTypeParam>,
    pub params: Vec<IrParam>,
    pub return_ty: IrType,
    /// `None` for abstract or interface methods.
    pub body: Option<Vec<IrStmt>>,
    pub throws: Vec<String>,
}

/// A constructor declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrConstructor {
    pub visibility: Visibility,
    pub params: Vec<IrParam>,
    pub body: Vec<IrStmt>,
    pub throws: Vec<String>,
}

/// A formal parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrParam {
    pub name: String,
    pub ty: IrType,
    pub is_varargs: bool,
}

/// An interface declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrInterface {
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<IrTypeParam>,
    pub extends: Vec<String>,
    pub methods: Vec<IrMethod>,
}

/// A single enum constant, e.g. `MERCURY(3.303e+23, 2.4397e6)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrEnumConstant {
    pub name: String,
    /// Constructor arguments (empty for simple enums).
    pub args: Vec<crate::IrExpr>,
    /// Per-constant method overrides (constant-specific class body).
    #[serde(default)]
    pub body: Vec<IrMethod>,
}

/// An enum declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrEnum {
    pub name: String,
    pub visibility: Visibility,
    /// Interfaces this enum implements.
    pub interfaces: Vec<String>,
    /// The enum constants in declaration order.
    pub constants: Vec<IrEnumConstant>,
    /// Instance fields (populated from field declarations inside the enum body).
    pub fields: Vec<IrField>,
    /// Instance and static methods defined on the enum.
    pub methods: Vec<IrMethod>,
    /// Constructor (at most one supported; private by convention).
    pub constructor: Option<IrConstructor>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip_class() {
        let cls = IrDecl::Class(IrClass {
            name: "HelloWorld".into(),
            visibility: Visibility::Public,
            is_abstract: false,
            is_final: false,
            type_params: vec![],
            superclass: None,
            interfaces: vec![],
            fields: vec![],
            methods: vec![IrMethod {
                name: "main".into(),
                visibility: Visibility::Public,
                is_static: true,
                is_abstract: false,
                is_final: false,
                type_params: vec![],
                params: vec![IrParam {
                    name: "args".into(),
                    ty: IrType::Array(Box::new(IrType::String)),
                    is_varargs: false,
                }],
                return_ty: IrType::Void,
                body: Some(vec![]),
                throws: vec![],
                is_synchronized: false,
            }],
            constructors: vec![],
        });
        let json = serde_json::to_string_pretty(&cls).unwrap();
        let back: IrDecl = serde_json::from_str(&json).unwrap();
        assert_eq!(cls, back);
    }
}
