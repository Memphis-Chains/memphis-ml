;; ============================================================
;; Rain Detector — automatic protective actions
;; ============================================================
;; Detects rain via moisture sensor on roof.
;; Automatically: closes windows, activates gutter heaters,
;; alerts homeowner, and prevents irrigation.
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (raining, wind OK):
;;   [ML] === Rain Detector ===
;;   [ML] RAIN DETECTED
;;   [ML] Closing all windows
;;   [Mock] gate 'window_all' -> close
;;   [Mock] gate 'gutter_heater' -> on
;;   [Mock] gate 'irrigation' -> off
;;   [ML] Irrigation automatically disabled — it's raining

(let raining true)
(let wind_speed 25)      ;; km/h
(let wind_threshold 80)
(let irrigation_auto true)
(let alarm_enabled true)

(log "=== Rain Detector ===")
(log raining)

(if (== raining true)
    (begin
        (log "RAIN DETECTED")
        (gate window_all close)
        (gate gutter_heater on)
        (gate skylight close)
        (log "All openings secured")
        (if (== irrigation_auto true)
            (begin
                (log "Irrigation automatically disabled — it's raining")
                (gate irrigation off)))
        (if (> wind_speed wind_threshold)
            (begin
                (log "HIGH WIND — be aware of conditions")
                (gate wind_warning on))
            (begin
                (log "Wind speed OK")
                (gate wind_warning off))))
    (begin
        (log "No rain")
        (gate gutter_heater off)
        (gate wind_warning off)
        (if (== irrigation_auto true)
            (gate irrigation on)))))

(if (== alarm_enabled true)
    (if (== raining true)
        (log "Rain alert sent to homeowner")))
