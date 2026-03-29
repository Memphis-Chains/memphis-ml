//! MQTT hardware backend for ML sensors and actuators.
//!
//! Uses the [`rumqttc`] async MQTT client to communicate with an MQTT broker.
//! Sensors subscribe to topics and cache readings; actuators publish commands
//! and query state via dedicated topics.
//!
//! # Topic Convention (ML naming)
//!
//! **Sensors**
//! - subscribe: `ml/sensors/{kind}/{id}/set`  → incoming readings
//! - kind = `temp`, `humidity`, `state`
//!
//! **Actuators**
//! - publish:    `ml/actuators/{id}/set`      → set power level
//! - subscribe:  `ml/actuators/{id}/state`   → current power level
//!
//! # Example
//!
//! ```rust,no_run
//! use ml_hw_mqtt::MqttBackend;
//! use ml_hal::Sensor;
//!
//! let mut backend = MqttBackend::new("mqtt://localhost:1883", "ml-client").unwrap();
//! let temp = backend.read_temp("temp.living_room").await;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

use ml_hal::{Actuator, HardwareError, Sensor, SensorKind};

/// MQTT client errors.
#[derive(Debug, thiserror::Error)]
pub enum MqttError {
    #[error("MQTT client error: {0}")]
    Client(#[from] rumqttc::ClientError),

    #[error("mqttc Error: {0}")]
    Mqttc(#[from] rumqttc::Error),

    #[error("timeout waiting for value on topic {0}")]
    Timeout(String),

    #[error("topic not found: {0}")]
    TopicNotFound(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// MQTT-based sensor/actuator backend.
///
/// Wraps a [`rumqttc::AsyncClient`] and maintains in-memory caches for
/// sensor readings and actuator states received over MQTT.
#[derive(Debug)]
pub struct MqttBackend {
    /// The underlying async MQTT client.
    client: rumqttc::AsyncClient,
    /// Incoming event channel, cloned from the client.
    eventloop: Arc<RwLock<Option<rumqttc::EventLoop>>>,
    /// Cached sensor values: topic → f64 / bool.
    sensor_cache: Arc<RwLock<HashMap<String, SensorValue>>>,
    /// Cached actuator power levels: actuator_id → f64.
    actuator_cache: Arc<RwLock<HashMap<String, f64>>>,
    /// Client identifier.
    client_id: String,
}

impl MqttBackend {
    /// Connect to an MQTT broker.
    ///
    /// `broker_url` – e.g. `mqtt://localhost:1883`
    /// `client_id`  – unique client identifier
    pub fn new(broker_url: impl Into<String>, client_id: impl Into<String>) -> Result<Self, MqttError> {
        let broker_url = broker_url.into();
        let client_id = client_id.into();

        let (client, eventloop) = rumqttc::AsyncClient::new(
            rumqttc::MqttOptions::new(&client_id, &broker_url, 1883),
            100, // max pending pub/sub packets
        );

        Ok(Self {
            client: client.clone(),
            eventloop: Arc::new(RwLock::new(Some(eventloop))),
            sensor_cache: Arc::new(RwLock::new(HashMap::new())),
            actuator_cache: Arc::new(RwLock::new(HashMap::new())),
            client_id,
        })
    }

    /// Set up subscriptions for all known sensor topics.
    ///
    /// Call this after adding sensors via the builder pattern or manually.
    /// Sensor IDs follow the ML convention:
    /// - `temp.<id>`       → subscribes to `ml/sensors/temp/<id>/set`
    /// - `humidity.<id>`   → subscribes to `ml/sensors/humidity/<id>/set`
    /// - `state.<id>`      → subscribes to `ml/sensors/state/<id>/set`
    pub async fn subscribe_sensors(&self, sensor_ids: &[&str]) -> Result<(), MqttError> {
        let mut topics = Vec::new();

        for &id in sensor_ids {
            let kind = sensor_kind_from_id(id);
            let topic = format!("ml/sensors/{}/{}/set", kind, id);
            topics.push((topic, rumqttc::QoS::AtLeastOnce));
        }

        self.client.subscribe_many(&topics).await?;
        log::debug!(target: "ml-hw-mqtt", "subscribed to sensor topics: {:?}", topics);
        Ok(())
    }

    /// Set up subscriptions for actuator state topics.
    ///
    /// For each actuator id, subscribes to `ml/actuators/{id}/state`.
    pub async fn subscribe_actuators(&self, actuator_ids: &[&str]) -> Result<(), MqttError> {
        let mut topics = Vec::new();

        for &id in actuator_ids {
            let topic = format!("ml/actuators/{}/state", id);
            topics.push((topic, rumqttc::QoS::AtLeastOnce));
        }

        self.client.subscribe_many(&topics).await?;
        log::debug!("subscribed to actuator topics: client_id={}, topics={:?}", self.client_id, topics);
        Ok(())
    }

    /// Start the background event-loop task that processes incoming packets.
    ///
    /// This must be spawned and kept alive for the cache to be populated.
    /// Returns a `JoinHandle` — drop it to stop the loop.
    pub fn spawn_event_loop(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut eventloop_opt = self.eventloop.write().await.take();

            if let Some(mut el) = eventloop_opt {
                loop {
                    let event = el.poll().await;

                    match event {
                        Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) => {
                            self.handleIncomingPublish(&publish).await;
                        }
                        Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))) => {
                            log::debug!("connected to MQTT broker: client_id={}", self.client_id);
                        }
                        Ok(rumqttc::Event::Outgoing(rumqttc::Packet::PingReq)) => {
                            // keep-alive tick — normal
                        }
                        Err(e) => {
                            log::warn!("MQTT event loop error: client_id={}, error={}", self.client_id, e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        _ => {}
                    }
                }
            }
        })
    }

    /// Handle an incoming PUBLISH packet — route it to the appropriate cache.
    async fn handleIncomingPublish(&self, publish: &rumqttc::Publish) {
        let topic = &publish.topic;
        let payload = &publish.payload;

        if topic.starts_with("ml/sensors/") {
            // Payload: JSON { "value": <f64 or bool> }
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(payload) {
                let value = SensorValue::from_json(json);
                let mut cache = self.sensor_cache.write().await;
                cache.insert(topic.clone(), value);
                log::trace!("cached sensor value: topic={}", topic);
            }
        } else if topic.starts_with("ml/actuators/") && topic.ends_with("/state") {
            // Extract actuator id from topic: ml/actuators/{id}/state
            if let Some(id) = parse_actuator_id_from_state_topic(topic) {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(payload) {
                    if let Some(level) = json["level"].as_f64() {
                        let mut cache = self.actuator_cache.write().await;
                        cache.insert(id, level);
                        log::trace!("cached actuator state: actuator_id={}, level={}", id, level);
                    }
                }
            }
        }
    }

    /// Wait for a value to arrive on a specific topic (used internally for req/resp).
    async fn wait_for_topic(&self, topic: &str, timeout_ms: u64) -> Result<SensorValue, MqttError> {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);

        let mut eventloop_opt = self.eventloop.write().await;
        if let Some(ref mut el) = *eventloop_opt {
            while tokio::time::Instant::now() < deadline {
                if let Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) =
                    timeout(Duration::from_millis(100), el.poll()).await
                {
                    if publish.topic == topic {
                        drop(eventloop_opt);
                        let json: serde_json::Value =
                            serde_json::from_slice(&publish.payload)?;
                        return Ok(SensorValue::from_json(json));
                    }
                }
            }
        }

        // Fallback: check cache
        let cache = self.sensor_cache.read().await;
        if let Some(v) = cache.get(topic) {
            return Ok(v.clone());
        }

        Err(MqttError::Timeout(topic.to_string()).into())
    }

    /// Publish an actuator power command.
    ///
    /// Publishes to `ml/actuators/{id}/set` with JSON `{ "level": <f64> }`.
    pub async fn publish_actuator(&self, id: &str, level: f64) -> Result<(), MqttError> {
        let topic = format!("ml/actuators/{}/set", id);
        let payload = serde_json::json!({ "level": level });
        self.client
            .publish(&topic, rumqttc::QoS::AtLeastOnce, false, payload.to_string())
            .await?;

        log::debug!("published actuator command: actuator_id={}, level={}", id, level);
        Ok(())
    }

    /// Manually refresh an actuator's state by publishing a GET request.
    ///
    /// Sends an empty message to `ml/actuators/{id}/get` (if supported by the broker).
    /// Then waits for the reply on `ml/actuators/{id}/state`.
    pub async fn refresh_actuator_state(&self, id: &str, timeout_ms: u64) -> Result<f64, HardwareError> {
        let state_topic = format!("ml/actuators/{}/state", id);

        // Publish a "get" request so the device responds with its current state
        let get_topic = format!("ml/actuators/{}/get", id);
        self.client
            .publish(&get_topic, rumqttc::QoS::AtLeastOnce, false, "{}")
            .await
            .map_err(|e| HardwareError::Io(e.to_string()))?;

        let value = timeout(
            Duration::from_millis(timeout_ms),
            self.wait_for_actuator_state(&state_topic),
        )
        .await
        .map_err(|_| HardwareError::Timeout)?
        .map_err(|e| HardwareError::Io(e.to_string()))?;

        Ok(value)
    }

    async fn wait_for_actuator_state(&self, topic: &str) -> Result<f64, MqttError> {
        let cache = self.actuator_cache.read().await;
        if let Some(&level) = cache.get(topic) {
            return Ok(level);
        }
        drop(cache);

        let mut eventloop_opt = self.eventloop.write().await;
        if let Some(ref mut el) = *eventloop_opt {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            while tokio::time::Instant::now() < deadline {
                if let Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) =
                    timeout(Duration::from_millis(100), el.poll()).await
                {
                    if publish.topic == topic {
                        let json: serde_json::Value = serde_json::from_slice(&publish.payload)?;
                        return json["level"]
                            .as_f64()
                            .ok_or_else(|| MqttError::Parse(format!("level not a number: {}", json["level"])));
                    }
                }
            }
        }

        Err(MqttError::Timeout(topic.to_string()))
    }

    /// Build a sensor topic string from an ML sensor id.
    fn sensor_topic(id: &str) -> String {
        let kind = sensor_kind_from_id(id);
        format!("ml/sensors/{}/{}/set", kind, id)
    }
}

/// Cached sensor value.
#[derive(Debug, Clone)]
enum SensorValue {
    Float(f64),
    Bool(bool),
}

impl SensorValue {
    fn from_json(json: serde_json::Value) -> Self {
        if let Some(v) = json["value"].as_f64() {
            SensorValue::Float(v)
        } else if let Some(v) = json["value"].as_bool() {
            SensorValue::Bool(v)
        } else {
            SensorValue::Float(0.0)
        }
    }
}

/// Determine sensor kind from an ML sensor id prefix.
fn sensor_kind_from_id(id: &str) -> &'static str {
    if id.starts_with("temp.") {
        "temp"
    } else if id.starts_with("humidity.") {
        "humidity"
    } else {
        "state"
    }
}

/// Extract actuator id from a state topic like `ml/actuators/{id}/state`.
fn parse_actuator_id_from_state_topic(topic: &str) -> Option<String> {
    let prefix = "ml/actuators/";
    if !topic.starts_with(prefix) {
        return None;
    }
    let rest = &topic[prefix.len()..];
    rest.strip_suffix("/state").map(|s| s.to_string())
}

// ─── Sensor trait ─────────────────────────────────────────────────────────

impl Sensor for MqttBackend {
    fn read_temp(&mut self, id: &str) -> Result<f64, HardwareError> {
        let topic = MqttBackend::sensor_topic(id);
        let cache = self.sensor_cache.blocking_read();

        match cache.get(&topic) {
            Some(SensorValue::Float(v)) => Ok(*v),
            Some(SensorValue::Bool(_)) => Err(HardwareError::Unavailable(
                format!("sensor '{}' is bool, not temp", id),
            )),
            None => Err(HardwareError::Unavailable(format!(
                "no cached value for '{}' (topic '{}') — ensure sensor is publishing",
                id, topic
            ))),
        }
    }

    fn read_humidity(&mut self, id: &str) -> Result<f64, HardwareError> {
        let topic = MqttBackend::sensor_topic(id);
        let cache = self.sensor_cache.blocking_read();

        match cache.get(&topic) {
            Some(SensorValue::Float(v)) => Ok(*v),
            Some(SensorValue::Bool(_)) => Err(HardwareError::Unavailable(
                format!("sensor '{}' is bool, not humidity", id),
            )),
            None => Err(HardwareError::Unavailable(format!(
                "no cached value for '{}' (topic '{}')",
                id, topic
            ))),
        }
    }

    fn read_bool(&mut self, id: &str) -> Result<bool, HardwareError> {
        let topic = MqttBackend::sensor_topic(id);
        let cache = self.sensor_cache.blocking_read();

        match cache.get(&topic) {
            Some(SensorValue::Bool(v)) => Ok(*v),
            Some(SensorValue::Float(_)) => Err(HardwareError::Unavailable(
                format!("sensor '{}' is float, not bool", id),
            )),
            None => Err(HardwareError::Unavailable(format!(
                "no cached value for '{}' (topic '{}')",
                id, topic
            ))),
        }
    }

    fn supports(&self, id: &str, kind: SensorKind) -> bool {
        match kind {
            SensorKind::Temperature => id.starts_with("temp."),
            SensorKind::Humidity => id.starts_with("humidity."),
            SensorKind::Bool => id.starts_with("state."),
            SensorKind::Custom(_) => true,
        }
    }
}

// ─── Actuator trait ────────────────────────────────────────────────────────

impl Actuator for MqttBackend {
    fn owns(&self, id: &str) -> bool {
        // Accepts any id — actuator ownership is determined by subscriptions
        true
    }

    fn set_power(&mut self, id: &str, level: f64) -> Result<(), HardwareError> {
        let topic = format!("ml/actuators/{}/set", id);
        let payload = serde_json::json!({ "level": level });
        let client = self.client.clone();

        // Fire-and-forget: spawn the async publish without blocking
        tokio::task::spawn(async move {
            if let Err(e) = client
                .publish(&topic, rumqttc::QoS::AtLeastOnce, false, payload.to_string())
                .await
            {
                log::warn!("set_power publish failed for {}: {}", topic, e);
            }
        });

        log::debug!("set_power via MQTT: actuator_id={}, level={}", id, level);
        Ok(())
    }

    fn get_power(&mut self, id: &str) -> Result<f64, HardwareError> {
        let state_topic = format!("ml/actuators/{}/state", id);
        let cache = self.actuator_cache.blocking_read();

        match cache.get(&state_topic).copied() {
            Some(v) => Ok(v),
            None => Err(HardwareError::Unavailable(format!(
                "no cached state for actuator '{}' (topic '{}') — make sure the device is publishing its state",
                id, state_topic
            ))),
        }
    }
}

// ─── Builder ────────────────────────────────────────────────────────────────

/// Builder for [`MqttBackend`].
#[derive(Debug, Default)]
pub struct MqttBuilder {
    broker_url: Option<String>,
    client_id: Option<String>,
    sensor_ids: Vec<String>,
    actuator_ids: Vec<String>,
}

impl MqttBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the MQTT broker URL (required).
    pub fn broker_url(mut self, url: impl Into<String>) -> Self {
        self.broker_url = Some(url.into());
        self
    }

    /// Set the client identifier (required).
    pub fn client_id(mut self, id: impl Into<String>) -> Self {
        self.client_id = Some(id.into());
        self
    }

    /// Add a temperature sensor id (ML convention: `temp.<id>`).
    pub fn with_temp_sensor(mut self, id: impl Into<String>) -> Self {
        self.sensor_ids.push(id.into());
        self
    }

    /// Add a humidity sensor id (ML convention: `humidity.<id>`).
    pub fn with_humidity_sensor(mut self, id: impl Into<String>) -> Self {
        self.sensor_ids.push(id.into());
        self
    }

    /// Add a boolean sensor id (ML convention: `state.<id>`).
    pub fn with_bool_sensor(mut self, id: impl Into<String>) -> Self {
        self.sensor_ids.push(id.into());
        self
    }

    /// Add an actuator id.
    pub fn with_actuator(mut self, id: impl Into<String>) -> Self {
        self.actuator_ids.push(id.into());
        self
    }

    /// Consume the builder and produce a connected [`MqttBackend`].
    pub async fn build(self) -> Result<Arc<MqttBackend>, MqttError> {
        let broker_url = self
            .broker_url
            .ok_or_else(|| MqttError::TopicNotFound("broker_url not set".into()))?;
        let client_id = self
            .client_id
            .ok_or_else(|| MqttError::TopicNotFound("client_id not set".into()))?;

        let backend = Arc::new(MqttBackend::new(&broker_url, &client_id)?);

        // Subscribe sensor topics
        if !self.sensor_ids.is_empty() {
            let sensor_refs: Vec<&str> = self.sensor_ids.iter().map(|s| s.as_str()).collect();
            backend.subscribe_sensors(&sensor_refs).await?;
        }

        // Subscribe actuator state topics
        if !self.actuator_ids.is_empty() {
            let actuator_refs: Vec<&str> = self.actuator_ids.iter().map(|s| s.as_str()).collect();
            backend.subscribe_actuators(&actuator_refs).await?;
        }

        // Spawn the event loop
        backend.clone().spawn_event_loop();

        Ok(backend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensor_kind_from_id() {
        assert_eq!(sensor_kind_from_id("temp.living_room"), "temp");
        assert_eq!(sensor_kind_from_id("humidity.bathroom"), "humidity");
        assert_eq!(sensor_kind_from_id("state.door_front"), "state");
        assert_eq!(sensor_kind_from_id("some_other.prefix"), "state"); // fallback
    }

    #[test]
    fn test_sensor_topic() {
        assert_eq!(
            MqttBackend::sensor_topic("temp.living_room"),
            "ml/sensors/temp/temp.living_room/set"
        );
        assert_eq!(
            MqttBackend::sensor_topic("humidity.bathroom"),
            "ml/sensors/humidity/humidity.bathroom/set"
        );
    }

    #[test]
    fn test_parse_actuator_id_from_state_topic() {
        assert_eq!(
            parse_actuator_id_from_state_topic("ml/actuators/fan.living_room/state"),
            Some("fan.living_room".to_string())
        );
        assert_eq!(parse_actuator_id_from_state_topic("ml/actuators/relay1/state"), Some("relay1".to_string()));
        assert_eq!(parse_actuator_id_from_state_topic("ml/sensors/temp/living/set"), None);
    }

    #[test]
    fn test_sensor_value_from_json() {
        let json = serde_json::json!({ "value": 23.5 });
        assert!(matches!(SensorValue::from_json(json), SensorValue::Float(23.5)));

        let json2 = serde_json::json!({ "value": true });
        assert!(matches!(SensorValue::from_json(json2), SensorValue::Bool(true)));

        let json3 = serde_json::json!({ "level": 0.8 });
        assert!(matches!(SensorValue::from_json(json3), SensorValue::Float(0.0))); // fallback
    }

    #[test]
    fn test_mqtt_builder_compiles() {
        // Builder requires broker_url and client_id — this test just checks it compiles
        let _builder = MqttBuilder::new()
            .broker_url("mqtt://localhost:1883")
            .client_id("test-client")
            .with_temp_sensor("temp.test1")
            .with_humidity_sensor("humidity.test1")
            .with_bool_sensor("state.door1")
            .with_actuator("actuator.fan1");
    }
}
