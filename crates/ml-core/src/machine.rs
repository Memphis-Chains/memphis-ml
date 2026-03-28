// ML-Core Machine — abstrakcja urządzeń

use crate::ast::{GateState, MLExpr, MLValue};
use crate::error::RuntimeError;
use std::collections::HashMap;

/// Abstrakcja maszyny — implementuj żeby sterować prawdziwym hardwarem
pub trait Machine {
    fn set_gate(&mut self, id: &str, state: GateState) -> Result<(), RuntimeError>;
    fn read_sensor(&mut self, sensor: &str) -> Result<f64, RuntimeError>;
}

/// Maszyna mock — dla testów i symulacji
#[derive(Default)]
pub struct MockMachine {
    gates: HashMap<String, GateState>,
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

    pub fn with_sensor(mut self, name: &str, value: f64) -> Self {
        self.sensors.insert(name.to_string(), value);
        self
    }
}

impl Machine for MockMachine {
    fn set_gate(&mut self, id: &str, state: GateState) -> Result<(), RuntimeError> {
        println!("[Mock] gate '{}' -> {:?}", id, state);
        self.gates.insert(id.to_string(), state);
        Ok(())
    }

    fn read_sensor(&mut self, sensor: &str) -> Result<f64, RuntimeError> {
        self.sensors
            .get(sensor)
            .copied()
            .ok_or_else(|| RuntimeError::SensorNotFound(sensor.to_string()))
    }
}

/// Runtime — wykonuje MLExpr na maszynie
pub struct Runtime<M: Machine> {
    machine: M,
    vars: HashMap<String, MLValue>,
}

impl<M: Machine> Runtime<M> {
    pub fn new(machine: M) -> Self {
        Self { machine, vars: HashMap::new() }
    }

    pub fn execute(&mut self, expr: MLExpr) -> Result<MLValue, RuntimeError> {
        self.eval(expr)
    }

    fn eval(&mut self, expr: MLExpr) -> Result<MLValue, RuntimeError> {
        match expr {
            MLExpr::Gate { id, state } => {
                self.machine.set_gate(&id, state)?;
                Ok(MLValue::Unit)
            }
            MLExpr::Read { sensor } => {
                let val = self.machine.read_sensor(&sensor)?;
                Ok(MLValue::Number(val))
            }
            MLExpr::Sequence(exprs) => {
                let mut result = MLValue::Unit;
                for expr in exprs {
                    result = self.eval(expr)?;
                }
                Ok(result)
            }
            MLExpr::If { condition, then_branch, else_ } => {
                if self.eval_condition(condition)? {
                    self.eval(*then_branch)
                } else if let Some(else_branch) = else_ {
                    self.eval(*else_branch)
                } else {
                    Ok(MLValue::Unit)
                }
            }
            MLExpr::Wait { ms } => {
                std::thread::sleep(std::time::Duration::from_millis(ms));
                Ok(MLValue::Unit)
            }
            MLExpr::Log { message } => {
                println!("[ML] {}", message);
                Ok(MLValue::Unit)
            }
            MLExpr::Let { name, value, body } => {
                let val = self.eval(*value)?;
                self.vars.insert(name.clone(), val);
                let result = self.eval(*body)?;
                self.vars.remove(&name);
                Ok(result)
            }
            MLExpr::Var(name) => {
                self.vars
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| RuntimeError::UndefinedVariable(name))
            }
            MLExpr::Bool(b) => Ok(MLValue::Bool(b)),
            MLExpr::Number(n) => Ok(MLValue::Number(n)),
            MLExpr::String(s) => Ok(MLValue::String(s)),
        }
    }

    fn eval_condition(&mut self, cond: crate::ast::Condition) -> Result<bool, RuntimeError> {
        match cond {
            crate::ast::Condition::Bool(b) => Ok(b),
            crate::ast::Condition::Eq(l, r) => {
                let lv = self.eval_value(*l)?;
                let rv = self.eval_value(*r)?;
                Ok(lv == rv)
            }
            crate::ast::Condition::Gt(l, r) => {
                let lv = self.eval_value(*l)?;
                let rv = self.eval_value(*r)?;
                match (lv, rv) {
                    (MLValue::Number(a), MLValue::Number(b)) => Ok(a > b),
                    _ => Err(RuntimeError::TypeMismatch("numbers for >".to_string())),
                }
            }
            crate::ast::Condition::Lt(l, r) => {
                let lv = self.eval_value(*l)?;
                let rv = self.eval_value(*r)?;
                match (lv, rv) {
                    (MLValue::Number(a), MLValue::Number(b)) => Ok(a < b),
                    _ => Err(RuntimeError::TypeMismatch("numbers for <".to_string())),
                }
            }
            crate::ast::Condition::And(l, r) => {
                Ok(self.eval_condition(*l)? && self.eval_condition(*r)?)
            }
            crate::ast::Condition::Or(l, r) => {
                Ok(self.eval_condition(*l)? || self.eval_condition(*r)?)
            }
            crate::ast::Condition::Not(inner) => Ok(!self.eval_condition(*inner)?),
        }
    }

    fn eval_value(&mut self, v: crate::ast::MLValue) -> Result<MLValue, RuntimeError> {
        match v {
            MLValue::Var(name) => {
                self.vars
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| RuntimeError::UndefinedVariable(name))
            }
            MLValue::Sensor(s) => {
                let val = self.machine.read_sensor(&s)?;
                Ok(MLValue::Number(val))
            }
            MLValue::Gate(_) => Err(RuntimeError::TypeMismatch("gate in value".to_string())),
            MLValue::Number(n) => Ok(MLValue::Number(n)),
            MLValue::String(s) => Ok(MLValue::String(s)),
            MLValue::Bool(b) => Ok(MLValue::Bool(b)),
            MLValue::Unit => Ok(MLValue::Unit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_gate() {
        let machine = MockMachine::new();
        let mut runtime = Runtime::new(machine);
        let expr = MLExpr::parse("(gate garage on)").unwrap();
        let result = runtime.execute(expr);
        assert!(result.is_ok());
    }

    #[test]
    fn execute_read() {
        let machine = MockMachine::new();
        let mut runtime = Runtime::new(machine);
        let expr = MLExpr::parse("(read temp.living_room)").unwrap();
        let result = runtime.execute(expr).unwrap();
        assert_eq!(result, MLValue::Number(22.5));
    }
}
