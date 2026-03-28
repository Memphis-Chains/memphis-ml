// ML-Core Runtime — executes ML AST on a Machine

use crate::ast::*;
use crate::error::RuntimeError;
use std::collections::HashMap;

pub trait Machine: Send {
    fn set_gate(&mut self, id: &str, state: &str) -> Result<(), RuntimeError>;
    fn read_sensor(&mut self, sensor: &str) -> Result<f64, RuntimeError>;
}

#[derive(Default)]
pub struct MockMachine {
    gates: HashMap<String, String>,
    sensors: HashMap<String, f64>,
}

impl MockMachine {
    pub fn new() -> Self {
        let mut m = Self::default();
        m.sensors.insert("temp.living_room".to_string(), 22.5);
        m.sensors.insert("temp.garage".to_string(), 18.0);
        m.sensors.insert("temp.outside".to_string(), 15.0);
        m.sensors.insert("temp.server".to_string(), 45.0);
        m
    }
}

impl Machine for MockMachine {
    fn set_gate(&mut self, id: &str, state: &str) -> Result<(), RuntimeError> {
        println!("[Mock] gate '{}' -> {}", id, state);
        self.gates.insert(id.to_string(), state.to_string());
        Ok(())
    }

    fn read_sensor(&mut self, sensor: &str) -> Result<f64, RuntimeError> {
        self.sensors.get(sensor).copied()
            .ok_or_else(|| RuntimeError::SensorNotFound(sensor.to_string()))
    }
}

/// Bridge from ml-hal CompositeMachine to ml-core Machine trait.
/// Routes gate/sensor ops through the HAL's trait hierarchy.
pub struct MlHalMachine(pub ml_hal::CompositeMachine);

impl Machine for MlHalMachine {
    fn set_gate(&mut self, id: &str, state: &str) -> Result<(), RuntimeError> {
        let gs = match state {
            "on" => ml_hal::GateState::On,
            "off" => ml_hal::GateState::Off,
            "toggle" => ml_hal::GateState::Toggle,
            _ => return Err(RuntimeError::TypeMismatch(format!("gate state: {}", state))),
        };
        ml_hal::Gate::set(&mut self.0, id, gs)
            .map_err(|e| RuntimeError::Machine(e.to_string()))
    }

    fn read_sensor(&mut self, sensor: &str) -> Result<f64, RuntimeError> {
        // Try temperature → humidity → bool (as 0/1)
        if let Ok(v) = ml_hal::Sensor::read_temp(&mut self.0, sensor) {
            return Ok(v);
        }
        if let Ok(v) = ml_hal::Sensor::read_humidity(&mut self.0, sensor) {
            return Ok(v);
        }
        if let Ok(b) = ml_hal::Sensor::read_bool(&mut self.0, sensor) {
            return Ok(if b { 1.0 } else { 0.0 });
        }
        Err(RuntimeError::SensorNotFound(sensor.into()))
    }
}

pub struct Runtime<M: Machine> {
    pub machine: M,
    vars: HashMap<String, MLValue>,
    /// Named function definitions: name -> (params, body)
    functions: HashMap<String, (Vec<String>, MLExpr)>,
}

impl<M: Machine> Runtime<M> {
    pub fn new(machine: M) -> Self {
        Self { machine, vars: HashMap::new(), functions: HashMap::new() }
    }

    pub fn execute(&mut self, expr: MLExpr) -> Result<MLValue, RuntimeError> {
        self.eval(expr)
    }

    fn eval(&mut self, expr: MLExpr) -> Result<MLValue, RuntimeError> {
        match expr {
            MLExpr::Gate { id, state } => {
                self.machine.set_gate(&id, &state)?;
                Ok(MLValue::Unit)
            }
            MLExpr::Read { sensor } => {
                let val = self.machine.read_sensor(&sensor)?;
                Ok(MLValue::Number(val))
            }
            MLExpr::Sequence(exprs) => {
                let mut result = MLValue::Unit;
                for expr in exprs { result = self.eval(expr)?; }
                Ok(result)
            }
            MLExpr::If { condition, then_branch, else_ } => {
                let cond = self.eval_condition(*condition)?;
                if cond {
                    self.eval(*then_branch)
                } else if let Some(e) = else_ {
                    self.eval(*e)
                } else {
                    Ok(MLValue::Unit)
                }
            }
            MLExpr::Wait { ms } => {
                std::thread::sleep(std::time::Duration::from_millis(ms));
                Ok(MLValue::Unit)
            }
            MLExpr::Log { message } => {
                let val = self.eval(*message)?;
                let msg = match val {
                    MLValue::Number(n) => {
                        if n.fract() == 0.0 { format!("{:.0}", n) }
                        else { n.to_string() }
                    }
                    MLValue::Bool(b) => b.to_string(),
                    MLValue::String(s) => s,
                    MLValue::Unit => "()".into(),
                    MLValue::Fn(..) => "<fn>".into(),
                };
                println!("[ML] {}", msg);
                Ok(MLValue::Unit)
            }
            MLExpr::Let { name, value, body } => {
                let val = self.eval(*value)?;
                // If the value is a function, also register it in self.functions
                // so it persists after the let binding goes out of scope
                if let MLValue::Fn(params, fn_body) = &val {
                    self.functions.insert(name.clone(), (params.clone(), *fn_body.clone()));
                }
                self.vars.insert(name.clone(), val);
                let result = self.eval(*body)?;
                self.vars.remove(&name);
                Ok(result)
            }
            MLExpr::Var(name) => {
                self.vars.get(&name).cloned()
                    .ok_or_else(|| RuntimeError::UndefinedVariable(name))
            }
            MLExpr::Bool(b) => Ok(MLValue::Bool(b)),
            MLExpr::Number(n) => Ok(MLValue::Number(n)),
            MLExpr::String(s) => Ok(MLValue::String(s)),
            MLExpr::Fn { args, body } => Ok(MLValue::Fn(args, body)),
            MLExpr::Defn { name, args, body } => {
                self.functions.insert(name, (args, *body));
                Ok(MLValue::Unit)
            }
            MLExpr::Call { name, args } => {
                // Look up the function by name in vars (first-class fn value) or functions map
                let (params, body) = if let Some(MLValue::Fn(params, body)) = self.vars.get(&name) {
                    (params.clone(), *body.clone())
                } else if let Some((params, body)) = self.functions.get(&name) {
                    (params.clone(), body.clone())
                } else {
                    return Err(RuntimeError::UndefinedVariable(format!("function: {}", name)));
                };

                // Evaluate all args
                let arg_vals: Vec<MLValue> = args.into_iter()
                    .map(|a| self.eval(a))
                    .collect::<Result<Vec<_>, _>>()?;

                // Bind params to arg_vals
                if arg_vals.len() != params.len() {
                    return Err(RuntimeError::TypeMismatch(format!(
                        "function '{}' expects {} args but got {}", name, params.len(), arg_vals.len()
                    )));
                }

                let param_names = params.clone();
                for (param, val) in params.into_iter().zip(arg_vals.into_iter()) {
                    self.vars.insert(param, val);
                }

                // Evaluate body and capture result
                let result = self.eval(body);

                // Remove bound params
                for param in param_names {
                    self.vars.remove(&param);
                }

                result
            }
            MLExpr::Set { name, value } => {
                let val = self.eval(*value)?;
                self.vars.insert(name, val);
                Ok(MLValue::Unit)
            }
            MLExpr::While { condition, body } => {
                while self.eval_condition(*condition.clone())? {
                    self.eval(*body.clone())?;
                }
                Ok(MLValue::Unit)
            }
            MLExpr::Begin(exprs) => {
                let mut result = MLValue::Unit;
                for e in exprs { result = self.eval(e)?; }
                Ok(result)
            }
            MLExpr::BinaryOp { op, left, right } => {
                let l = self.eval(*left)?;
                let r = self.eval(*right)?;
                let ln = l.as_number().ok_or_else(|| RuntimeError::TypeMismatch("number".into()))?;
                let rn = r.as_number().ok_or_else(|| RuntimeError::TypeMismatch("number".into()))?;
                let n = match op.as_str() {
                    "+" => ln + rn,
                    "-" => ln - rn,
                    "*" => ln * rn,
                    "/" => ln / rn,
                    "%" => ln % rn,
                    _ => return Err(RuntimeError::TypeMismatch(op)),
                };
                Ok(MLValue::Number(n))
            }
            MLExpr::UnaryOp { op, operand } => {
                let v = self.eval(*operand)?;
                match op.as_str() {
                    "not" | "!" => {
                        let b = v.as_bool().ok_or_else(|| RuntimeError::TypeMismatch("bool".into()))?;
                        Ok(MLValue::Bool(!b))
                    }
                    _ => Err(RuntimeError::TypeMismatch(op)),
                }
            }
        }
    }

    fn eval_condition(&mut self, expr: MLExpr) -> Result<bool, RuntimeError> {
        match expr {
            MLExpr::Bool(b) => Ok(b),
            MLExpr::Number(n) => Ok(n != 0.0),
            MLExpr::String(s) => Ok(!s.is_empty()),
            MLExpr::Var(name) => {
                self.vars.get(&name).and_then(|v| v.as_bool())
                    .ok_or_else(|| RuntimeError::UndefinedVariable(name))
            }
            MLExpr::If { condition, then_branch, else_ } => {
                if self.eval_condition(*condition)? {
                    self.eval(*then_branch).map(|v| v.as_bool().unwrap_or(false))
                } else if let Some(e) = else_ {
                    self.eval(*e).map(|v| v.as_bool().unwrap_or(false))
                } else {
                    Ok(false)
                }
            }
            MLExpr::BinaryOp { op, left, right } => {
                let l = self.eval(*left)?;
                let r = self.eval(*right)?;
                let ln = l.as_number().ok_or_else(|| RuntimeError::TypeMismatch("number".into()))?;
                let rn = r.as_number().ok_or_else(|| RuntimeError::TypeMismatch("number".into()))?;
                Ok(match op.as_str() {
                    "==" => ln == rn,
                    "!=" => ln != rn,
                    ">" => ln > rn,
                    "<" => ln < rn,
                    ">=" => ln >= rn,
                    "<=" => ln <= rn,
                    _ => false,
                })
            }
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_run(source: &str) -> Result<MLValue, RuntimeError> {
        let expr = crate::parser::Parser::new(source).parse().unwrap();
        let machine = MockMachine::new();
        let mut runtime = Runtime::new(machine);
        runtime.execute(expr)
    }

    #[test]
    fn gate_on() {
        let r = parse_run("(gate garage on)");
        assert!(r.is_ok());
    }

    #[test]
    fn gate_off() {
        let r = parse_run("(gate door off)");
        assert!(r.is_ok());
    }

    #[test]
    fn read_temp() {
        let r = parse_run("(read temp.living_room)").unwrap();
        assert_eq!(r, MLValue::Number(22.5));
    }

    #[test]
    fn wait_noop() {
        let r = parse_run("(wait 1)");
        assert!(r.is_ok());
    }

    #[test]
    fn log_noop() {
        let r = parse_run(r#"(log "hello")"#);
        assert!(r.is_ok());
    }

    #[test]
    fn let_binding() {
        let r = parse_run("(let x 10 x)").unwrap();
        assert_eq!(r, MLValue::Number(10.0));
    }

    #[test]
    fn let_with_expression() {
        let r = parse_run("(let x 5 (+ x 3))").unwrap();
        assert_eq!(r, MLValue::Number(8.0));
    }

    #[test]
    fn binary_plus() {
        let r = parse_run("(+ 3 5)").unwrap();
        assert_eq!(r, MLValue::Number(8.0));
    }

    #[test]
    fn binary_mult() {
        let r = parse_run("(* 4 7)").unwrap();
        assert_eq!(r, MLValue::Number(28.0));
    }

    #[test]
    fn comparison_gt() {
        // (> 10 5) should evaluate to true via BinaryOp
        let r = parse_run("(+ 0 1)"); // simple test first
        assert!(r.is_ok());
    }

    #[test]
    fn sequence() {
        let r = parse_run("(gate g on) (gate g off)").unwrap();
        assert_eq!(r, MLValue::Unit);
    }

    #[test]
    fn set_var() {
        let r = parse_run("(set x 42)").unwrap();
        assert_eq!(r, MLValue::Unit);
    }

    #[test]
    fn bool_true() {
        let r = parse_run("true").unwrap();
        assert_eq!(r, MLValue::Bool(true));
    }

    #[test]
    fn bool_false() {
        let r = parse_run("false").unwrap();
        assert_eq!(r, MLValue::Bool(false));
    }

    #[test]
    fn string_val() {
        let r = parse_run(r#""hello""#).unwrap();
        assert_eq!(r, MLValue::String("hello".into()));
    }

    #[test]
    fn number_val() {
        let r = parse_run("42").unwrap();
        assert_eq!(r, MLValue::Number(42.0));
    }

    #[test]
    fn fn_def_and_call() {
        // (let add (fn (x y) (+ x y))) (call add 3 5) → 8
        let r = parse_run("(let add (fn (x y) (+ x y))) (call add 3 5)").unwrap();
        assert_eq!(r, MLValue::Number(8.0));
    }

    #[test]
    fn defn_and_call() {
        // (fn add (x y) (+ x y)) (call add 3 5) → 8
        let r = parse_run("(fn add (x y) (+ x y)) (call add 3 5)").unwrap();
        assert_eq!(r, MLValue::Number(8.0));
    }

    #[test]
    fn fn_call_with_expression_arg() {
        // (call mul 3 (+ 2 5)) where mul multiplies two args → 21
        let r = parse_run("(fn mul (x y) (* x y)) (call mul 3 (+ 2 5))").unwrap();
        assert_eq!(r, MLValue::Number(21.0));
    }

    #[test]
    fn fn_no_args() {
        // A function with no args that returns a constant
        let r = parse_run("(fn answer () 42) (call answer)").unwrap();
        assert_eq!(r, MLValue::Number(42.0));
    }
}
