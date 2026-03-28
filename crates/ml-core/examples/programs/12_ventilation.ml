;; ============================================================
;; Ventilation Controller — CO2-based air quality control
;; ============================================================
;; Activates ventilation when CO2 exceeds safe levels.
;; Level < 800ppm: good, < 1000ppm: moderate, >= 1000ppm: poor.
;; Uses: let, if, gate, log, begin, binary ops, read
;;
;; Expected output (co2=950):
;;   [ML] CO2 level: 950 ppm
;;   [ML] CO2 MODERATE — opening windows
;;   [Mock] gate 'exhaust_fan' -> on
;;   [Mock] gate 'window_east' -> open

(let co2_level 950)
(let hvac_mode true)

(log "=== Ventilation Controller ===")
(log "CO2 level:")
(log co2_level)

(if (< co2_level 800)
    (begin
        (log "Air quality: GOOD")
        (gate exhaust_fan off)
        (gate window_east off)
        (gate window_west off)
        (log "Ventilation off"))
    (if (< co2_level 1000)
        (begin
            (log "CO2 MODERATE — opening windows")
            (gate exhaust_fan on)
            (gate window_east open)
            (gate window_west off)
            (log "Partial ventilation active"))
        (begin
            (log "CO2 POOR — full ventilation blast!")
            (gate exhaust_fan on)
            (gate window_east open)
            (gate window_west open)
            (gate hvac_boost on)
            (if (== hvac_mode true)
                (begin
                    (log "HVAC on fresh air recirculation override")
                    (gate hvac_fresh_air on)))
            (log "MAXIMUM VENTILATION"))))
