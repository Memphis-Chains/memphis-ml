;; ============================================================
;; Irrigation System — soil moisture based automation
;; ============================================================
;; Waters plants when soil is dry AND not raining AND not too hot.
;; Uses: let, if, gate, log, begin, binary ops, read
;;
;; Expected output (soil dry, no rain, temp OK):
;;   [ML] Soil moisture: 28%
;;   [ML] Soil is DRY — starting irrigation zone 1
;;   [Mock] gate 'valve_zone1' -> on
;;   [Mock] gate 'valve_zone2' -> off

(let soil_moisture 28)     ;; percent, simulated
(let raining false)
(let ambient_temp 28)      ;; celsius

(log "=== Irrigation Controller ===")
(log soil_moisture)
(log raining)

(if (< soil_moisture 30)
    (begin
        (if (== raining false)
            (begin
                (if (< ambient_temp 35)
                    (begin
                        (log "Soil is DRY — starting irrigation zone 1")
                        (gate valve_zone1 on)
                        (gate valve_zone2 off))
                    (begin
                        (log "Too hot to irrigate — risk of leaf burn")
                        (gate valve_zone1 off)))
                (log "Irrigation scheduled for early morning"))
            (begin
                (log "It is raining — skipping irrigation")
                (gate valve_zone1 off)))
        (if (> soil_moisture 60)
            (log "Soil still wet from yesterday")))
    (begin
        (log "Soil moisture OK")
        (gate valve_zone1 off)
        (gate valve_zone2 off)))
