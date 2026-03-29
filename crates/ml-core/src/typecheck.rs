// ML-Core Typechecker — infers and checks types for ML expressions
//
// Type inference is done via a simple constraint-solving walk of the AST.
// Each expression produces a Type; constraints are accumulated and unified.
// Cyclic constraints (e.g. recursive functions) are handled via occurs-check.

use crate::ast::MLExpr;
use std::collections::HashMap;

/// ML types inferred during typechecking
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Numeric values (f64)
    Number,
    /// Booleans
    Bool,
    /// String values
    String,
    /// Unit / void / nil
    Unit,
    /// Function type: (arg_types..., return_type)
    Fn(Vec<Type>, Box<Type>),
    /// A type variable — unifies with anything during inference
    Var(String),
    /// Type error / unresolved
    Unknown,
}

impl Type {
    /// Pretty-print a type
    pub fn show(&self) -> String {
        match self {
            Type::Number => "Number".into(),
            Type::Bool => "Bool".into(),
            Type::String => "String".into(),
            Type::Unit => "Unit".into(),
            Type::Var(v) => format!("t{}", v),
            Type::Fn(args, ret) => {
                let args_str = args.iter().map(|a| a.show()).collect::<Vec<_>>().join(" -> ");
                format!("({args_str}) -> {}", ret.show())
            }
            Type::Unknown => "Unknown".into(),
        }
    }
}

/// A type constraint: two types must be equal
#[derive(Debug, Clone)]
pub struct Constraint {
    pub left: Type,
    pub right: Type,
    pub reason: String,
}

/// The typechecking environment
pub struct TypeEnv {
    /// Inferred / assumed types for variables
    vars: HashMap<String, Type>,
    /// Inferred / assumed types for functions
    functions: HashMap<String, Type>,
    /// Next fresh type variable ID
    next_var: usize,
    /// Accumulated constraints
    constraints: Vec<Constraint>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            functions: HashMap::new(),
            next_var: 0,
            constraints: Vec::new(),
        }
    }

    /// Allocate a fresh type variable
    fn fresh(&mut self) -> Type {
        let id = self.next_var;
        self.next_var += 1;
        Type::Var(format!("t{}", id))
    }

    /// Add a type constraint
    fn constrain(&mut self, left: Type, right: Type, reason: String) {
        self.constraints.push(Constraint { left, right, reason });
    }

    /// Infer the type of an expression
    pub fn infer(&mut self, expr: &MLExpr) -> Type {
        match expr {
            MLExpr::Number(_) => Type::Number,
            MLExpr::String(_) => Type::String,
            MLExpr::Bool(_) => Type::Bool,
            MLExpr::Nil => Type::Unit,
            MLExpr::Gate { .. } => Type::Unit,
            MLExpr::Read { .. } => Type::Number,
            MLExpr::Wait { .. } => Type::Unit,
            MLExpr::Log { .. } => Type::Unit,
            MLExpr::Var(name) => {
                self.vars.get(name).cloned().unwrap_or_else(|| {
                    let tv = self.fresh();
                    self.vars.insert(name.clone(), tv.clone());
                    tv
                })
            }
            MLExpr::Let { name, value, body } => {
                let vt = self.infer(value);
                self.vars.insert(name.clone(), vt);
                let bt = self.infer(body);
                self.vars.remove(name);
                bt
            }
            MLExpr::Set { name, value } => {
                let vt = self.infer(value);
                if let Some(t) = self.vars.get(name) {
                    self.constrain(t.clone(), vt.clone(), format!("set! {} must maintain type", name));
                } else {
                    self.vars.insert(name.clone(), vt.clone());
                }
                vt
            }
            MLExpr::If { condition, then_branch, else_ } => {
                let ct = self.infer(condition);
                self.constrain(ct, Type::Bool, "if condition must be Bool".into());
                let tt = self.infer(then_branch);
                if let Some(eb) = else_ {
                    let et = self.infer(eb);
                    self.constrain(tt.clone(), et.clone(), "if/else branch type mismatch".into());
                    tt
                } else {
                    self.constrain(tt.clone(), Type::Unit, "if without else must return Unit".into());
                    tt
                }
            }
            MLExpr::While { condition, body } => {
                let ct = self.infer(condition);
                self.constrain(ct, Type::Bool, "while condition must be Bool".into());
                let bt = self.infer(body);
                self.constrain(bt, Type::Unit, "while body should return Unit".into());
                Type::Unit
            }
            MLExpr::Fn { args, body } => {
                let arg_types: Vec<Type> = args.iter().map(|a| {
                    let t = self.fresh();
                    self.vars.insert(a.clone(), t.clone());
                    t
                }).collect();
                let ret_type = self.infer(body);
                // Remove arg bindings (they're local to the fn)
                for arg in args {
                    self.vars.remove(arg);
                }
                Type::Fn(arg_types, Box::new(ret_type))
            }
            MLExpr::Defn { name, args, body } => {
                let arg_types: Vec<Type> = args.iter().map(|a| {
                    let t = self.fresh();
                    self.vars.insert(a.clone(), t.clone());
                    t
                }).collect();
                let ret_type = self.infer(body);
                for arg in args {
                    self.vars.remove(arg);
                }
                let fn_type = Type::Fn(arg_types, Box::new(ret_type.clone()));
                self.functions.insert(name.clone(), fn_type.clone());
                self.vars.insert(name.clone(), fn_type);
                Type::Unit
            }
            MLExpr::Call { name, args } => {
                let arg_types: Vec<Type> = args.iter().map(|a| self.infer(a)).collect();
                // Try to get the function type from env — clone fn_type first to avoid borrow conflict
                let fn_type_opt = self.functions.get(name).cloned();
                if let Some(Type::Fn(expected_args, ret_box)) = fn_type_opt {
                    if expected_args.len() == arg_types.len() {
                        let arg_constraint_fmts: Vec<(Type, Type, String)> = expected_args
                            .iter()
                            .zip(arg_types.iter())
                            .map(|(ea, at)| (ea.clone(), at.clone(), format!("arg type mismatch in call to {}", name)))
                            .collect();
                        // Now do mutable operations (constrain)
                        for (ea, at, fmt) in arg_constraint_fmts {
                            self.constrain(ea, at, fmt);
                        }
                        // Extract Type from Box<Type> via explicit as_ref()
                        return (*ret_box.as_ref()).clone();
                    }
                }
                // Fall back to treating as a first-class call with unknown return
                let ret = self.fresh();
                self.functions.insert(name.clone(), Type::Fn(arg_types.clone(), Box::new(ret.clone())));
                ret
            }
            MLExpr::Sequence(exprs) | MLExpr::Begin(exprs) => {
                exprs.last().map(|e| self.infer(e)).unwrap_or(Type::Unit)
            }
            MLExpr::BinaryOp { op, left, right } => {
                let lt = self.infer(left);
                let rt = self.infer(right);
                match op.as_str() {
                    "+" | "-" | "*" | "/" | "%" => {
                        self.constrain(lt.clone(), Type::Number, "+/-/*///% requires Number".into());
                        self.constrain(rt.clone(), Type::Number, "+/-/*///% requires Number".into());
                        Type::Number
                    }
                    "==" | "!=" | ">" | "<" | ">=" | "<=" => {
                        self.constrain(lt, rt, "comparison operands must have same type".into());
                        Type::Bool
                    }
                    "and" | "or" => {
                        self.constrain(lt, Type::Bool, "and/or requires Bool".into());
                        self.constrain(rt, Type::Bool, "and/or requires Bool".into());
                        Type::Bool
                    }
                    _ => Type::Unknown,
                }
            }
            MLExpr::UnaryOp { op, operand } => {
                let ot = self.infer(operand);
                match op.as_str() {
                    "not" | "!" => {
                        self.constrain(ot, Type::Bool, "not requires Bool".into());
                        Type::Bool
                    }
                    "-" => {
                        self.constrain(ot, Type::Number, "unary - requires Number".into());
                        Type::Number
                    }
                    _ => Type::Unknown,
                }
            }
            MLExpr::Return(e) => self.infer(e),
        }
    }

    /// Unify all accumulated constraints, returning the final type substitution map.
    /// Returns a map of type variable names -> resolved types.
    pub fn unify(&mut self) -> HashMap<String, Type> {
        let mut subst = HashMap::new();
        // Collect new constraints to add separately to avoid borrow conflict
        let mut new_constraints: Vec<Constraint> = Vec::new();

        for constraint in std::mem::take(&mut self.constraints) {
            let left = self.apply_subst(&constraint.left, &subst);
            let right = self.apply_subst(&constraint.right, &subst);
            if let Type::Var(v) = &left {
                if !self.occurs_in(&v, &right) {
                    subst.insert(v.clone(), right.clone());
                }
            } else if let Type::Var(v) = &right {
                if !self.occurs_in(&v, &left) {
                    subst.insert(v.clone(), left.clone());
                }
            }
            // Structural unification for Fn types
            if let (Type::Fn(a1, r1), Type::Fn(a2, r2)) = (&left, &right) {
                if a1.len() == a2.len() {
                    let reason = constraint.reason.clone();
                    for (t1, t2) in a1.iter().zip(a2.iter()) {
                        new_constraints.push(Constraint {
                            left: t1.clone(),
                            right: t2.clone(),
                            reason: reason.clone(),
                        });
                    }
                    new_constraints.push(Constraint {
                        left: *r1.clone(),
                        right: *r2.clone(),
                        reason,
                    });
                }
            }
        }

        // Put the new constraints back
        self.constraints = new_constraints;
        subst
    }

    /// Apply current substitution to a type
    fn apply_subst(&self, t: &Type, subst: &HashMap<String, Type>) -> Type {
        match t {
            Type::Var(v) => subst.get(v).cloned().unwrap_or_else(|| t.clone()),
            Type::Fn(args, ret) => {
                Type::Fn(
                    args.iter().map(|a| self.apply_subst(a, subst)).collect(),
                    Box::new(self.apply_subst(ret, subst)),
                )
            }
            _ => t.clone(),
        }
    }

    /// Check if a type variable occurs in a type (for occurs-check)
    fn occurs_in(&self, var: &str, t: &Type) -> bool {
        match t {
            Type::Var(v) => v == var,
            Type::Fn(args, ret) => {
                args.iter().any(|a| self.occurs_in(var, a)) || self.occurs_in(var, ret)
            }
            _ => false,
        }
    }

    /// Full pipeline: infer types and apply substitution to get concrete types.
    /// Returns (inferred_type, substitution_map)
    pub fn check(&mut self, expr: &MLExpr) -> (Type, HashMap<String, Type>) {
        let t = self.infer(expr);
        let subst = self.unify();
        (self.apply_subst(&t, &subst), subst)
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Typecheck a source string and return the inferred type as a string.
pub fn check_source(source: &str) -> Result<String, String> {
    let expr = crate::parser::Parser::new(source)
        .parse()
        .map_err(|e| format!("parse error: {}", e))?;
    let mut env = TypeEnv::new();
    let (t, _) = env.check(&expr);
    Ok(t.show())
}

/// Typecheck an MLExpr and return the inferred type.
pub fn check_expr(expr: &MLExpr) -> String {
    let mut env = TypeEnv::new();
    let (t, _) = env.check(expr);
    t.show()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_number() {
        let env = &mut TypeEnv::new();
        let t = env.infer(&MLExpr::Number(42.0));
        assert_eq!(t, Type::Number);
    }

    #[test]
    fn infer_bool() {
        let env = &mut TypeEnv::new();
        let t = env.infer(&MLExpr::Bool(true));
        assert_eq!(t, Type::Bool);
    }

    #[test]
    fn infer_binary_op() {
        let expr = MLExpr::BinaryOp {
            op: "+".into(),
            left: Box::new(MLExpr::Number(1.0)),
            right: Box::new(MLExpr::Number(2.0)),
        };
        let env = &mut TypeEnv::new();
        let t = env.infer(&expr);
        assert_eq!(t, Type::Number);
    }

    #[test]
    fn infer_fn() {
        let expr = MLExpr::Fn {
            args: vec!["x".into()],
            body: Box::new(MLExpr::BinaryOp {
                op: "+".into(),
                left: Box::new(MLExpr::Var("x".into())),
                right: Box::new(MLExpr::Number(1.0)),
            }),
        };
        let env = &mut TypeEnv::new();
        let t = env.infer(&expr);
        match t {
            Type::Fn(args, ret) => {
                assert_eq!(args.len(), 1);
                assert_eq!(*ret, Type::Number);
            }
            _ => panic!("expected Fn type, got {:?}", t),
        }
    }

    #[test]
    fn infer_let() {
        let expr = MLExpr::Let {
            name: "x".into(),
            value: Box::new(MLExpr::Number(10.0)),
            body: Box::new(MLExpr::Var("x".into())),
        };
        let env = &mut TypeEnv::new();
        let t = env.infer(&expr);
        assert_eq!(t, Type::Number);
    }

    #[test]
    fn infer_if() {
        let expr = MLExpr::If {
            condition: Box::new(MLExpr::Bool(true)),
            then_branch: Box::new(MLExpr::Number(1.0)),
            else_: Some(Box::new(MLExpr::Number(2.0))),
        };
        let env = &mut TypeEnv::new();
        let t = env.infer(&expr);
        assert_eq!(t, Type::Number);
    }

    #[test]
    fn infer_nested_fn() {
        // (fn (x) (fn (y) (+ x y))) — closure capturing x
        let expr = MLExpr::Fn {
            args: vec!["x".into()],
            body: Box::new(MLExpr::Fn {
                args: vec!["y".into()],
                body: Box::new(MLExpr::BinaryOp {
                    op: "+".into(),
                    left: Box::new(MLExpr::Var("x".into())),
                    right: Box::new(MLExpr::Var("y".into())),
                }),
            }),
        };
        let env = &mut TypeEnv::new();
        let t = env.infer(&expr);
        match t {
            Type::Fn(args, ret) => {
                assert_eq!(args.len(), 1);
                assert_eq!(*ret, Type::Fn(_, _));
            }
            _ => panic!("expected Fn type, got {:?}", t),
        }
    }

    #[test]
    fn check_source_basic() {
        let result = check_source("(+ 1 2)");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Number");
    }

    #[test]
    fn check_source_fn() {
        let result = check_source("(fn (x) (+ x 1))");
        assert!(result.is_ok());
        // Should be (Number) -> Number
        assert!(result.unwrap().contains("Number"));
    }
}
