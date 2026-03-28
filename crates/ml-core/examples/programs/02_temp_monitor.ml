;; ============================================================
;; Temperature Monitor — multi-room monitoring with alerts
;; ============================================================
;; Reads three temperature sensors and triggers alarms
;; if any room exceeds safe thresholds.
;; Uses: let, read, if, gate, log, begin
;;
;; Expected output:
;;   [ML] Living room: 22.5
;;   [ML] WARNING: Garage too hot!
;;   [Mock] gate 'garage_alarm' -> on
;;   [Mock] gate 'garage_fan' -> on

(let living_temp (read temp.living_room))
(let garage_temp (read temp.garage))
(let outside_temp (read temp.outside))

(log "=== Temperature Monitor ===")
(log living_temp)
(log garage_temp)
(log outside_temp)

(if (> garage_temp 30)
    (begin
        (log "WARNING: Garage too hot!")
        (gate garage_alarm on)
        (gate garage_fan on))
    (begin
        (log "Garage temp OK")
        (gate garage_fan off)))
