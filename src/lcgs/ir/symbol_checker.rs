use crate::lcgs::ast::{BinaryOpKind, DeclKind, Expr, ExprKind, Identifier, UnaryOpKind};
use crate::lcgs::ir::symbol_table::{Owner, SymbolIdentifier, SymbolTable};

/// [CheckMode]s control which declaration identifiers are allow to refer to in the [SymbolChecker].
#[derive(Eq, PartialEq)]
pub enum CheckMode {
    /// In Const mode, identifiers in expressions can only refer to constants.
    Const,
    /// In LabelOrTransition mode, identifiers in expressions can only refer to constants
    /// and state variables.
    LabelOrTransition,
    /// In StateVarChange mode, identifiers in expressions can only refer constants, state
    /// variables, and transitions (actions)
    StateVarChange,
}

impl CheckMode {
    /// Returns true if the declaration kind is allowed in this mode. See the definition of
    /// each mode for more details.
    pub fn allows(&self, decl_kind: &DeclKind) -> bool {
        match self {
            CheckMode::Const => matches!(decl_kind, DeclKind::Const(_)),
            CheckMode::LabelOrTransition => {
                matches!(decl_kind, DeclKind::Const(_) | DeclKind::StateVar(_))
            }
            CheckMode::StateVarChange => {
                matches!(decl_kind, DeclKind::Const(_) | DeclKind::StateVar(_) | DeclKind::Transition(_))
            }
        }
    }
}

/// A [SymbolChecker] will check identifiers in expressions and also optimize expressions
/// where possible. The [SymbolChecker] has multiple modes for different types of expressions.
pub struct SymbolChecker<'a> {
    symbols: &'a SymbolTable,
    scope_owner: Owner,
    mode: CheckMode,
}

impl<'a> SymbolChecker<'a> {
    pub fn new(symbols: &'a SymbolTable, scope_owner: Owner, mode: CheckMode) -> SymbolChecker<'a> {
        SymbolChecker {
            symbols,
            scope_owner,
            mode,
        }
    }

    /// Checks the given expressions
    pub fn check(&self, expr: &Expr) -> Result<Expr, ()> {
        match &expr.kind {
            ExprKind::Number(_) => Ok(expr.clone()),
            ExprKind::OwnedIdent(id) => self.check_ident(id),
            ExprKind::UnaryOp(op, expr) => self.check_unop(op, expr),
            ExprKind::BinaryOp(op, e1, e2) => self.check_binop(op, e1, e2),
            ExprKind::TernaryIf(c, e1, e2) => self.check_if(c, e1, e2),
        }
    }

    /// Checks the given owned identifier.
    fn check_ident(&self, id: &Identifier) -> Result<Expr, ()> {
        // Owner may be omitted. If omitted, we assume it is the scope owner, unless such thing
        // does not exist, then we assume it's global. If we still can't find it, we have an error.
        let symb = match id {
            // Simple identifiers are typically declaration names and should not appear in
            // expressions. They are resolved differently.
            Identifier::Simple { .. } => {
                panic!("Simple identifiers should not be resolved by SymbolChecker.")
            }
            Identifier::OptionalOwner { owner, name } => {
                // Constants are evaluated early, so we differentiate here to
                // give better error messages.
                if self.mode == CheckMode::Const {
                    if let Some(player_name) = owner {
                        // Constants are never owned by players
                        // TODO Use custom error
                        panic!("Expected constant expression. Found reference to player.")
                    } else {
                        self.symbols
                            .get(&Owner::Global, &name)
                            // TODO Use custom error
                            .expect("Expected constant expression. Found unknown constant.")
                            .borrow()
                    }
                } else {
                    if let Some(player_name) = owner {
                        // We first ensure that the player exists in order to give a
                        // more accurate error message, if necessary
                        self.symbols
                            .get(&Owner::Global, player_name)
                            .expect("Unknown player"); // TODO Use custom error

                        // The player exists, so now we fetch the symbol
                        let owner = Owner::Player(player_name.to_string());
                        self.symbols
                            .get(&owner, &name)
                                // TODO Use custom error
                            .expect("Unknown identifier. The player does not own a declaration of that name")
                    } else {
                        // Player is omitted. Assume it is scope owner. If not, then try global.
                        self.symbols
                            .get(&self.scope_owner, &name)
                            .or_else(|| self.symbols.get(&Owner::Global, &name))
                                // TODO Use custom error
                            .expect("Unknown identifier, neither declared locally or globally")
                    }
                    .try_borrow()
                        // If the try_borrow fails, it means that the RefCell is currently being
                        // mutated by someone. We are only reducing one declaration at a time,
                        // so the declaration must have an identifier referring to itself.
                    .expect("Declaration refers to itself.") // TODO Use custom error
                }
            }
            // Already resolved once ... which should never happen.
            Identifier::Resolved { .. } => panic!("Identifier was already resolved once."),
        };

        // Check if symbol is allow to be reference in this mode
        if !self.mode.allows(&symb.declaration.kind) {
            return Err(());
        }

        if let DeclKind::Const(con) = &symb.declaration.kind {
            // If symbol points to a constant declaration we can inline the value
            return self.check(&con.definition);
        }

        // Identifier is okay. Return a resolved identifier where owner is specified.
        let SymbolIdentifier { owner, name } = &symb.identifier;
        return Ok(Expr {
            kind: ExprKind::OwnedIdent(Box::new(Identifier::Resolved {
                owner: owner.clone(),
                name: name.clone(),
            })),
        });
    }

    /// Optimizes the given unary operator and checks the operand
    fn check_unop(&self, op: &UnaryOpKind, expr: &Expr) -> Result<Expr, ()> {
        let mut res = self.check(expr)?;
        if let ExprKind::Number(n) = &mut res.kind {
            match op {
                UnaryOpKind::Not => *n = -*n,
                UnaryOpKind::Negation => *n = (*n == 0) as i32,
            }
        }
        Ok(Expr {
            kind: ExprKind::UnaryOp(op.clone(), Box::new(res)),
        })
    }

    /// Optimizes the given binary operator and checks the operands
    fn check_binop(&self, op: &BinaryOpKind, e1: &Expr, e2: &Expr) -> Result<Expr, ()> {
        // Optimize if both operands are numbers
        // TODO Some operators allow optimizations even when only one operand is a number
        let res1 = self.check(e1)?;
        let res2 = self.check(e2)?;
        if let ExprKind::Number(n1) = &res1.kind {
            if let ExprKind::Number(n2) = &res2.kind {
                return Ok(Expr {
                    kind: ExprKind::Number(op.as_fn()(*n1, *n2)),
                });
            }
        }
        Ok(Expr {
            kind: ExprKind::BinaryOp(op.clone(), Box::new(res1), Box::new(res2)),
        })
    }

    /// Optimizes the given ternary if and checks the operands
    fn check_if(&self, cond: &Expr, e1: &Expr, e2: &Expr) -> Result<Expr, ()> {
        let cond_res = self.check(cond)?;
        if let ExprKind::Number(n) = &cond_res.kind {
            return if *n == 0 {
                self.check(e2)
            } else {
                self.check(e1)
            };
        }
        Ok(Expr {
            kind: ExprKind::TernaryIf(
                Box::new(cond_res),
                Box::new(self.check(e1)?),
                Box::new(self.check(e2)?),
            ),
        })
    }
}