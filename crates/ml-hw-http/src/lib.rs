use ml_hal::{Actuator, HardwareError, Sensor, SensorKind};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

pub struct HttpBackend {
    base_url: String,
    client: reqwest::blocking::Client,
    headers: HeaderMap,
}

impl HttpBackend {
    pub fn new(base_url: impl Into<String>) -> Self {
        HttpBackend {
            base_url: base_url.into(),
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap(),
            headers: HeaderMap::new(),
        }
    }

    pub fn with_auth(mut self, token: &str) -> Self {
        let name = HeaderName::try_from("authorization").ok();
        let val = HeaderValue::from_str(&format!("Bearer {}", token)).ok();
        if let (Some(n), Some(v)) = (name, val) {
            self.headers.insert(n, v);
        }
        self
    }

    fn get(&self, path: &str) -> Result<String, HardwareError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .headers(self.headers.clone())
            .send()
            .map_err(|e| HardwareError::Io(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(HardwareError::Unavailable(resp.status().to_string()));
        }
        resp.text()
            .map_err(|e| HardwareError::Io(e.to_string()))
    }

    fn post(&self, path: &str, body: &serde_json::Value) -> Result<(), HardwareError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .headers(self.headers.clone())
            .send()
            .map_err(|e| HardwareError::Io(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(HardwareError::Unavailable(resp.status().to_string()));
        }
        Ok(())
    }
}

impl Sensor for HttpBackend {
    fn read_temp(&mut self, id: &str) -> Result<f64, HardwareError> {
        let body: serde_json::Value = self
            .get(&format!("/sensors/{}/temp", id))?
            .parse()
            .map_err(|e| HardwareError::Io(format!("json parse error: {}", e)))?;
        body["value"]
            .as_f64()
            .ok_or_else(|| HardwareError::Unavailable("temp value not a number".into()))
    }

    fn read_humidity(&mut self, id: &str) -> Result<f64, HardwareError> {
        let body: serde_json::Value = self
            .get(&format!("/sensors/{}/humidity", id))?
            .parse()
            .map_err(|e| HardwareError::Io(format!("json parse error: {}", e)))?;
        body["value"]
            .as_f64()
            .ok_or_else(|| HardwareError::Unavailable("humidity value not a number".into()))
    }

    fn read_bool(&mut self, id: &str) -> Result<bool, HardwareError> {
        let body: serde_json::Value = self
            .get(&format!("/sensors/{}/state", id))?
            .parse()
            .map_err(|e| HardwareError::Io(format!("json parse error: {}", e)))?;
        body["value"]
            .as_bool()
            .ok_or_else(|| HardwareError::Unavailable("bool value not a boolean".into()))
    }

    fn supports(&self, _id: &str, _kind: SensorKind) -> bool {
        true // optimistic - will fail at read time if not supported
    }
}

impl Actuator for HttpBackend {
    fn set_power(&mut self, id: &str, level: f64) -> Result<(), HardwareError> {
        self.post(
            &format!("/actuators/{}/power", id),
            &serde_json::json!({ "level": level }),
        )
    }

    fn get_power(&mut self, id: &str) -> Result<f64, HardwareError> {
        let body: serde_json::Value = self
            .get(&format!("/actuators/{}/power", id))?
            .parse()
            .map_err(|e| HardwareError::Io(format!("json parse error: {}", e)))?;
        body["level"]
            .as_f64()
            .ok_or_else(|| HardwareError::Unavailable("power level not a number".into()))
    }

    fn owns(&self, _id: &str) -> bool {
        true
    }
}

// Note: Gate trait is not implemented for HttpBackend since HTTP can't do GPIO
// Use ml-hw-gpio for Gate control, HttpBackend for Sensor+Actuator
