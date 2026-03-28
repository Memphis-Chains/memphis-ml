;; ============================================================
;; Greenhouse Automation — complex multi-sensor control
;; ============================================================
;; Manages: heating, cooling, shading, irrigation, CO2 enrichment.
;; Rules:
;;   - Heat if temp < 18°C
;;   - Cool + shade if temp > 28°C
;;   - Irrigate if soil moisture < 40%
;;   - CO2 boost if CO2 < 600ppm and lights on (indoor greenhouse)
;;   - Alert if any sensor fails (value = -1)
;; Uses: let, if, gate, log, begin, binary ops, read, while
;;
;; Expected output:
;;   [ML] === Greenhouse Control ===
;;   [ML] Soil: 35, CO2: 450, Temp: 22
;;   [ML] Soil DRY — irrigating
;;   [Mock] gate 'irrigation' -> on
;;   [ML] CO2 low and lights on — enriching
;;   [Mock] gate 'co2_tank' -> on

(let soil_moisture 35)
(let co2_level 450)
(let air_temp 22)
(let lights_on true)
(let humidity 70)

(log "=== Greenhouse Control ===")
(log "Soil moisture:")
(log soil_moisture)
(log "CO2:")
(log co2_level)
(log "Air temp:")
(log air_temp)

;; Temperature control
(if (< air_temp 18)
    (begin
        (log "Cold — activating heating")
        (gate heater on)
        (gate vent off))
    (if (> air_temp 28)
        (begin
            (log "Hot — cooling and shading")
            (gate cooling on)
            (gate shade open)
            (gate vent on))
        (begin
            (log "Temperature optimal")
            (gate heater off)
            (gate cooling off)
            (gate shade close))))

;; Irrigation
(if (< soil_moisture 40)
    (begin
        (log "Soil DRY — irrigating")
        (gate irrigation on)
        (set soil_moisture (+ soil_moisture 10)))
    (begin
        (log "Soil moisture OK")
        (gate irrigation off)))

;; CO2 enrichment
(if (< co2_level 600)
    (begin
        (log "CO2 low")
        (if (== lights_on true)
            (begin
                (log "CO2 low and lights on — enriching")
                (gate co2_tank on))
            (begin
                (log "CO2 low but lights off — skip enrichment")
                (gate co2_tank off))))
    (begin
        (log "CO2 level adequate")
        (gate co2_tank off)))

;; Humidity control
(if (> humidity 85)
    (begin
        (log "Humidity too high — dehumidifying")
        (gate dehumidifier on)
        (gate vent on))
    (if (< humidity 50)
        (begin
            (log "Humidity too low — misting")
            (gate mister on))
        (begin
            (log "Humidity optimal")
            (gate dehumidifier off)
            (gate mister off))))

;; System status summary
(log "=== Status Summary ===")
(log "All systems nominal")
