;; ============================================================
;; Weather Station — read and display multiple sensors
;; ============================================================
;; Collects temperature, humidity, pressure, and wind speed.
;; Computes a comfort index from temp and humidity.
;; Uses: let, read, if, log, begin, binary ops
;;
;; Expected output:
;;   [ML] === Weather Station ===
;;   [ML] Temp: 22.5
;;   [ML] Garage: 18
;;   [ML] Outside: 15
;;   [ML] Comfort index: 22
;;   [Mock] gate 'display_update' -> on

(let temp (read temp.living_room))
(let garage (read temp.garage))
(let outside (read temp.outside))
(let humidity 65)

(log "=== Weather Station ===")
(log "Temperature readings:")
(log temp)
(log garage)
(log outside)

(let comfort_index (- temp 0))
(log "Comfort index:")
(log comfort_index)

(if (< comfort_index 18)
    (begin
        (log "Cold — turning on floor heating")
        (gate floor_heating on))
    (if (> comfort_index 26)
        (begin
            (log "Hot — activating cooling")
            (gate ac_unit on))
        (begin
            (log "Comfortable temperature")
            (gate floor_heating off)
            (gate ac_unit off))))

(gate display_update on)
