use core::fmt;
use std::fmt::{Display, Formatter};

use crate::lcgs::ast::BinaryOpKind::*;
use std::ops::{Add, Mul, Sub, Div};

/// The root of a LCGS program.
#[derive(Debug, Eq, PartialEq)]
pub struct Root {
    /// Declarations in the global scope. The parser ensures that only
    /// the allowed declaration types are present (e.g. not transitions).
    pub decls: Vec<Decl>,
}

/// A declaration
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Decl {
    pub kind: DeclKind,
}

/// Every kind of declaration
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum DeclKind {
    Const(Box<ConstDecl>),
    Label(Box<LabelDecl>),
    StateVar(Box<StateVarDecl>),
    StateVarChange(Box<StateVarChangeDecl>),
    Player(Box<PlayerDecl>),
    Template(Box<TemplateDecl>),
    Transition(Box<TransitionDecl>),
}

impl DeclKind {
    /// Returns the identifier of the declaration
    pub fn ident(&self) -> &Identifier {
        // This may seem a bit silly, but we don't want to lift the name out of the declarations
        match self {
            DeclKind::Const(decl) => &decl.name,
            DeclKind::Label(decl) => &decl.name,
            DeclKind::StateVar(decl) => &decl.name,
            DeclKind::StateVarChange(decl) => &decl.name,
            DeclKind::Player(decl) => &decl.name,
            DeclKind::Template(decl) => &decl.name,
            DeclKind::Transition(decl) => &decl.name,
        }
    }
}

/// An identifier with an optional owner, eg "`p1.health`". In this language we only ever
/// have two scopes, template or global. Templates are essentially player types, so once
/// instantiated, the player will own an instance of each declaration inside the template.
/// Hence, identifier inside expressions can refer to a specific owner (the name in front
/// of the dot in "`p1.health`". If the owner is omitted, the owner is either the current
/// template (if such declaration exists) or the global scope.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct OwnedIdentifier {
    /// The owner of the declaration, i.e. the name in from of the dot in "`p1.health`".
    /// None implies that the owner is the current template or global
    pub owner: Option<String>,
    pub name: String,
}

/// An identifier (with no explicit owner). This is typically the name of a declaration.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Identifier {
    pub name: String,
}

/// A constant. E.g. "`const max_health = 1`"
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ConstDecl {
    pub name: Identifier,
    pub definition: Expr,
}

/// A label declaration. Labels are also called propositions. Example: "`label alive = health > 0`"
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct LabelDecl {
    pub condition: Expr,
    pub name: Identifier,
}

/// A player declaration. A player based on a template with some optional relabelling.
/// E.g. "`player p1 = shooter [target1=p2, target2=p3]`"
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct PlayerDecl {
    pub name: Identifier,
    pub template: Identifier,
    pub relabeling: Relabeling,
}

/// A list of relabeling cases. Relabeling means replacing name with another name or a
/// number. This allows the user to slightly tweak a template. It can be thought of
/// as passing arguments to a template.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Relabeling {
    pub relabellings: Vec<RelabelCase>,
}

/// A relabeling case. Whenever the `prev_name` is found in the given template, it is
/// replaced with `new_name`.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RelabelCase {
    pub prev_name: Identifier,
    pub new_name: Identifier,
}

/// A template declaration. Essentially a player type.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TemplateDecl {
    pub name: Identifier,
    /// The parser ensures that only the allowed declaration kinds are present
    /// (not constants, players, or other templates)
    pub decls: Vec<Decl>,
    pub params: Vec<Param>,
}

/// A parameter to a template. I.e. something that must be relabeled.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Param {
    pub name: Identifier,
    pub typ: ParamType,
}

/// A parameter type. It is only possible to relabel to new identifiers or integers.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ParamType {
    IdentType(Identifier),
    IntType,
}

/// A variable declaration. The state of the CGS is the combination of all variables.
/// E.g. "`health : [0 .. max_health] init max_health`"
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct StateVarDecl {
    pub name: Identifier,
    pub range: TypeRange,
    pub initial_value: Expr,
}

/// A variable-change declaration. In this declaration the user defines how a variable
/// changes based on the previous state and the actions taken.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct StateVarChangeDecl {
    pub name: Identifier,
    pub next_value: Expr,
}

/// A range for state variables.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TypeRange {
    pub min: Expr,
    pub max: Expr,
}

/// A transition declaration defines what actions a player can take.
/// If the condition is not satisfied, then the player cannot take the action in the
/// current state. Transitions in a CGS is the combination of all
/// players' actions. Each player must have at least one action available to them.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TransitionDecl {
    pub name: Identifier,
    pub condition: Expr,
}

/// An expression. Expressions are always of type integer.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Expr {
    pub kind: ExprKind,
}

/// Every kind of expression
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ExprKind {
    Number(i32),
    OwnedIdent(Box<OwnedIdentifier>),
    UnaryOp(UnaryOpKind, Box<Expr>),
    BinaryOp(BinaryOpKind, Box<Expr>, Box<Expr>),
    TernaryIf(Box<Expr>, Box<Expr>, Box<Expr>),
}

/// Unary operators
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum UnaryOpKind {
    Not,
    Negation, // eg -4
}

impl UnaryOpKind {
    pub fn as_fun(&self) -> fn(i32) -> i32 {
        match self {
            UnaryOpKind::Not => |e| (e == 0) as i32,
            UnaryOpKind::Negation => |e| -e,
        }
    }
}

/// Binary operators
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum BinaryOpKind {
    Addition,
    Multiplication,
    Subtraction,
    Division,
    Equality, // Also serves as bi-implication
    Inequality,
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
    And,
    Or,
    Xor,
    Implication,
}

impl BinaryOpKind {
    pub fn as_fn(&self) -> fn(i32, i32) -> i32 {
        match self {
            BinaryOpKind::Addition => i32::add,
            BinaryOpKind::Multiplication => i32::mul,
            BinaryOpKind::Subtraction => i32::sub,
            BinaryOpKind::Division => i32::div,
            BinaryOpKind::Equality => |e1, e2| (e1 == e2) as i32,
            BinaryOpKind::Inequality => |e1, e2| (e1 != e2) as i32,
            BinaryOpKind::GreaterThan => |e1, e2| (e1 > e2) as i32,
            BinaryOpKind::LessThan => |e1, e2| (e1 < e2) as i32,
            BinaryOpKind::GreaterOrEqual => |e1, e2| (e1 >= e2) as i32,
            BinaryOpKind::LessOrEqual => |e1, e2| (e1 <= e2) as i32,
            BinaryOpKind::And => |e1, e2| (e1 != 0 && e2 != 0) as i32,
            BinaryOpKind::Or => |e1, e2| (e1 != 0 || e2 != 0) as i32,
            BinaryOpKind::Xor => |e1, e2| ((e1 == 0 && e2 != 0) || e1 != 0 && e2 == 0) as i32,
            BinaryOpKind::Implication => |e1, e2| (e1 == 0 || e2 != 0) as i32,
        }
    }
}

impl From<&[u8]> for BinaryOpKind {
    fn from(op: &[u8]) -> BinaryOpKind {
        match op {
            b"+" => Addition,
            b"*" => Multiplication,
            b"-" => Subtraction,
            b"/" => Division,
            b"==" => Equality,
            b"!=" => Inequality,
            b">" => GreaterThan,
            b"<" => LessThan,
            b">=" => GreaterOrEqual,
            b"<=" => LessOrEqual,
            b"&&" => And,
            b"||" => Or,
            b"^" => Xor,
            b"->" => Implication,
            _ => unimplemented!(
                "Unrecognized operator '{}'. See 'impl From<u8> for BinaryOpKind' clause.",
                String::from_utf8(op.to_vec()).unwrap()
            ),
        }
    }
}

impl Display for BinaryOpKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Addition => "+",
                Multiplication => "*",
                Subtraction => "-",
                Division => "/",
                Equality => "==",
                Inequality => "!=",
                GreaterThan => ">",
                LessThan => "<",
                GreaterOrEqual => ">=",
                LessOrEqual => "<=",
                And => "&&",
                Or => "||",
                Xor => "^",
                Implication => "->",
            }
        )
    }
}
