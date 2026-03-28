;; ============================================================
;; Energy Saver — scheduled non-essential load shedding
;; ============================================================
;; Peak hours (6-9 AM, 5-9 PM): shed washer, dryer, EV charger.
;; Off-peak: allow heavy loads.
;; Solar mode: allow all loads when panels produce surplus.
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (5 PM, non-solar peak):
;;   [ML] === Energy Saver ===
;;   [ML] Peak rate period — shedding heavy loads
;;   [Mock] gate 'washer' -> off
;;   [Mock] gate 'dryer' -> off
;;   [Mock] gate 'ev_charger' -> off

(let hour 17)
(let solar_surplus false)
(let is_peak false)

(log "=== Energy Saver ===")
(log "Hour:")
(log hour)

;; Determine if peak hour
(if (>= hour 6)
    (if (< hour 9)
        (begin
            (log "Morning peak (6-9 AM)")
            (set is_peak true))
        (if (>= hour 17)
            (if (< hour 21)
                (begin
                    (log "Evening peak (5-9 PM)")
                    (set is_peak true))))))

(if (== is_peak true)
    (begin
        (if (== solar_surplus true)
            (begin
                (log "Solar surplus — allowing some loads")
                (gate washer on)
                (gate dryer off)
                (gate ev_charger off))
            (begin
                (log "Peak rate period — shedding heavy loads")
                (gate washer off)
                (gate dryer off)
                (gate ev_charger off)
                (gate dishwasher on)
                (log "Dishwasher allowed (low power)"))))
    (begin
        (log "Off-peak — all loads allowed")
        (gate washer on)
        (gate dryer on)
        (gate ev_charger on)
        (gate dishwasher on))))

(log "Energy profile applied")
