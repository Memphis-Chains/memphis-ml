;; ============================================================
;; Heater Control — thermostat with hysteresis
;; ============================================================
;; Uses hysteresis to prevent short-cycling:
;;   turns ON when temp < setpoint - 1
;;   turns OFF when temp > setpoint + 1
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (temp=19, setpoint=21, hysteresis=1):
;;   [ML] Current: 19, Setpoint: 21
;;   [ML] Heater ON (below lower threshold 20.0)
;;   [Mock] gate 'heater' -> on

(let current_temp 19)
(let setpoint 21)
(let hysteresis 1)
(let heater_on false)

(log "=== Heater Control ===")
(log current_temp)
(log setpoint)

(let lower_threshold (- setpoint hysteresis))
(let upper_threshold (+ setpoint hysteresis))

(if (< current_temp lower_threshold)
    (begin
        (log "Heater ON (below lower threshold)")
        (log lower_threshold)
        (gate heater on)
        (set heater_on true))
    (if (> current_temp upper_threshold)
        (begin
            (log "Heater OFF (above upper threshold)")
            (log upper_threshold)
            (gate heater off)
            (set heater_on false))
        (begin
            (log "Heater maintaining — within hysteresis band")
            (log "No change needed"))))
