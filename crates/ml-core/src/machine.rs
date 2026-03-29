// ML-Core Runtime — executes ML AST on a Machine

use crate::ast::*;
use crate::error::RuntimeError;
use std::collections::HashMap;

/// Maximum loop iterations before the runtime aborts with "infinite loop".
const MAX_LOOP_ITERATIONS: usize = 1_000_000;

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
    /// Current scope variables
    vars: HashMap<String, MLValue>,
    /// Named function definitions: name -> (params, body)
    functions: HashMap<String, (Vec<String>, MLExpr)>,
    /// Stack of upvalue sets for closures. Each frame has captured upvars.
    upvar_stack: Vec<HashMap<String, MLValue>>,
    /// Loop depth — used to detect runaway loops
    loop_count: usize,
}

impl<M: Machine> Runtime<M> {
    pub fn new(machine: M) -> Self {
        Self {
            machine,
            vars: HashMap::new(),
            functions: HashMap::new(),
            upvar_stack: Vec::new(),
            loop_count: 0,
        }
    }

    /// Collect free variables from `body` that are:
    /// - NOT in `params` (function parameters)
    /// - NOT in `local_names` (let-bound variables in the current scope)
    /// - Present in `self.vars` (available in the enclosing scope)
    fn capture_upvars(&self, body: &MLExpr, params: &[String], local_names: &[String]) -> Vec<(String, UpvarKind)> {
        let mut captured = Vec::new();
        let free_vars = Self::free_vars(body, params, local_names);
        for var_name in free_vars {
            // Determine if this upvar is from the immediate parent or from a further outer scope
            let kind = if self.upvar_stack.len() == 1 {
                UpvarKind::Local(self.upvar_stack[0].len())
            } else {
                UpvarKind::Up(self.upvar_stack.len())
            };
            if self.vars.get(&var_name).is_some() {
                captured.push((var_name.clone(), kind.clone()));
                // Also store the captured value in a local upvar table
                // (we'll look it up by name at call time)
            }
        }
        captured
    }

    /// Compute the set of free variables (not locally bound) in an expression.
    fn free_vars(expr: &MLExpr, params: &[String], local_names: &[String]) -> Vec<String> {
        match expr {
            MLExpr::Var(v) => {
                if !params.contains(v) && !local_names.contains(v) {
                    vec![v.clone()]
                } else {
                    vec![]
                }
            }
            MLExpr::Number(_) | MLExpr::String(_) | MLExpr::Bool(_) | MLExpr::Nil => vec![],
            MLExpr::Gate { .. } | MLExpr::Read { .. } | MLExpr::Wait { .. } => vec![],
            MLExpr::Sequence(es) | MLExpr::Begin(es) => {
                let mut locals = local_names.to_vec();
                let mut result = Vec::new();
                for e in es {
                    let fvs = Self::free_vars(e, params, &locals);
                    result.extend(fvs);
                    // Update locals with any let bindings in this expr
                    if let MLExpr::Let { name, .. } = e {
                        locals.push(name.clone());
                    }
                }
                result
            }
            MLExpr::If { condition, then_branch, else_ } => {
                let mut result = Self::free_vars(condition, params, local_names);
                result.extend(Self::free_vars(then_branch, params, local_names));
                if let Some(e) = else_ {
                    result.extend(Self::free_vars(e, params, local_names));
                }
                result
            }
            MLExpr::Let { name, value, body } => {
                let mut fvs = Self::free_vars(value, params, local_names);
                let mut new_locals = local_names.to_vec();
                new_locals.push(name.clone());
                fvs.extend(Self::free_vars(body, params, &new_locals));
                fvs
            }
            MLExpr::Set { name, value } => {
                let mut fvs = Self::free_vars(value, params, local_names);
                if !params.contains(name) && !local_names.contains(name) {
                    fvs.push(name.clone());
                }
                fvs
            }
            MLExpr::While { condition, body } => {
                let mut fvs = Self::free_vars(condition, params, local_names);
                fvs.extend(Self::free_vars(body, params, local_names));
                fvs
            }
            MLExpr::Fn { args, body } => {
                let mut new_params = params.to_vec();
                new_params.extend(args.clone());
                Self::free_vars(body, &new_params, local_names)
            }
            MLExpr::Defn { args, body, .. } => {
                let mut new_params = params.to_vec();
                new_params.extend(args.clone());
                Self::free_vars(body, &new_params, local_names)
            }
            MLExpr::Call { name, args } => {
                let mut result = Vec::new();
                for a in args {
                    result.extend(Self::free_vars(a, params, local_names));
                }
                if !params.contains(name) && !local_names.contains(name) {
                    result.push(name.clone());
                }
                result
            }
            MLExpr::BinaryOp { left, right, .. } => {
                let mut result = Self::free_vars(left, params, local_names);
                result.extend(Self::free_vars(right, params, local_names));
                result
            }
            MLExpr::UnaryOp { operand, .. } => {
                Self::free_vars(operand, params, local_names)
            }
            MLExpr::Return(e) => Self::free_vars(e, params, local_names),
            MLExpr::Log { message } => Self::free_vars(message, params, local_names),
        }
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
                    MLValue::Fn(..) | MLValue::Closure(..) => "<fn>".into(),
                    MLValue::Nil => "nil".into(),
                    MLValue::Return(_) => "<return>".into(),
                };
                println!("[ML] {}", msg);
                Ok(MLValue::Unit)
            }
            MLExpr::Let { name, value, body } => {
                let val = self.eval(*value)?;
                // If the value is a function, also register it in self.functions
                if let MLValue::Fn(params, fn_body) = &val {
                    self.functions.insert(name.clone(), (params.clone(), *fn_body.clone()));
                }
                self.vars.insert(name.clone(), val);
                let result = self.eval(*body)?;
                self.vars.remove(&name);
                Ok(result)
            }
            MLExpr::Var(name) => {
                // Look in current vars, then in upvar stack (for closures)
                if let Some(v) = self.vars.get::<str>(&name) {
                    Ok(v.clone())
                } else {
                    // Check upvar stacks (captured closure variables)
                    for frame in &self.upvar_stack {
                        if let Some(v) = frame.get::<str>(&name) {
                            return Ok(v.clone());
                        }
                    }
                    Err(RuntimeError::UndefinedVariable(name))
                }
            }
            MLExpr::Bool(b) => Ok(MLValue::Bool(b)),
            MLExpr::Number(n) => Ok(MLValue::Number(n)),
            MLExpr::String(s) => Ok(MLValue::String(s)),
            MLExpr::Nil => Ok(MLValue::Nil),
            MLExpr::Fn { args, body } => {
                // Capture free variables from enclosing scope
                let upvars = self.capture_upvars(&body, &args, &[]);
                Ok(MLValue::Closure(Closure { args, body, upvars }))
            }
            MLExpr::Defn { name, args, body } => {
                self.functions.insert(name.clone(), (args.clone(), *body.clone()));
                Ok(MLValue::Unit)
            }
            MLExpr::Call { name, args } => {
                // Look up the function
                let fval = self.vars.get(&name)
                    .cloned()
                    .or_else(|| self.functions.get(&name)
                        .map(|(a, b): &(Vec<String>, MLExpr)| MLValue::Fn(a.clone(), Box::new(b.clone()))))
                    .ok_or_else(|| RuntimeError::UndefinedVariable(format!("function: {}", name)))?;

                let (params, body, upvars) = match fval {
                    MLValue::Fn(params, body) => (params, *body, None),
                    MLValue::Closure(c) => (c.args, *c.body, Some(c.upvars)),
                    _ => return Err(RuntimeError::TypeMismatch(format!("not a function: {}", name))),
                };

                // Evaluate all args
                let arg_vals: Vec<MLValue> = args.into_iter()
                    .map(|a| self.eval(a))
                    .collect::<Result<Vec<_>, _>>()?;

                if arg_vals.len() != params.len() {
                    return Err(RuntimeError::TypeMismatch(format!(
                        "function '{}' expects {} args but got {}", name, params.len(), arg_vals.len()
                    )));
                }

                // Push upvar frame for this closure call
                let prev_upvar_stack = self.upvar_stack.clone();
                self.upvar_stack.push(HashMap::new());
                if let Some(ref uvs) = upvars {
                    // Copy captured upvar values into the new frame
                    let frame: &mut HashMap<String, MLValue> = self.upvar_stack.last_mut().unwrap();
                    for (uv_name, _) in uvs {
                        if let Some(v) = self.vars.get::<str>(uv_name) {
                            frame.insert(uv_name.clone(), v.clone());
                        }
                    }
                }

                // Bind parameters
                let param_names: Vec<String> = params.clone();
                for (param, val) in params.into_iter().zip(arg_vals.into_iter()) {
                    self.vars.insert(param, val);
                }

                // Evaluate body and unwrap return value
                let result = self.eval(body);

                // Remove bound params and pop upvar frame
                for param in &param_names {
                    self.vars.remove(param);
                }
                self.upvar_stack.pop();

                // Restore previous upvar stack
                self.upvar_stack = prev_upvar_stack;

                // Handle early return
                match result {
                    Ok(MLValue::Return(v)) => Ok(*v),
                    other => other,
                }
            }
            MLExpr::Set { name, value } => {
                let val = self.eval(*value)?;
                // Check if this is an upvar we're setting (mutate captured variable)
                let mut found = false;
                for frame in &mut self.upvar_stack {
                    if frame.contains_key::<str>(&name) {
                        frame.insert(name.clone(), val.clone());
                        found = true;
                        break;
                    }
                }
                if !found {
                    self.vars.insert(name, val);
                }
                Ok(MLValue::Unit)
            }
            MLExpr::While { condition, body } => {
                self.loop_count = 0;
                while self.eval_condition(*condition.clone())? {
                    self.loop_count += 1;
                    if self.loop_count > MAX_LOOP_ITERATIONS {
                        return Err(RuntimeError::Machine(format!("infinite loop detected (>{MAX_LOOP_ITERATIONS} iterations)")));
                    }
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
            MLExpr::Return(e) => {
                // Wrap in Return sentinel — caller unwraps it
                Ok(MLValue::Return(Box::new(self.eval(*e)?)))
            }
        }
    }

    fn eval_condition(&mut self, expr: MLExpr) -> Result<bool, RuntimeError> {
        match expr {
            MLExpr::Bool(b) => Ok(b),
            MLExpr::Number(n) => Ok(n != 0.0),
            MLExpr::String(s) => Ok(!s.is_empty()),
            MLExpr::Var(name) => {
                self.vars.get(&name)
                    .and_then(|v: &MLValue| v.as_bool())
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
