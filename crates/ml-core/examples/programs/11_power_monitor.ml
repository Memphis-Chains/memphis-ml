;; ============================================================
;; Power Monitor — consumption tracking and alerts
;; ============================================================
;; Monitors total power draw. Sends alerts when approaching
;; the breaker limit (80%) and trips the main if overloaded (100%).
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (load=8200W, limit=10000W):
;;   [ML] === Power Monitor ===
;;   [ML] Total load: 8200W
;;   [ML] Load is HIGH — shedding non-essential loads
;;   [Mock] gate 'dryer' -> off
;;   [Mock] gate 'pool_pump' -> off

(let total_watts 8200)
(let limit_watts 10000)
(let dryer_running true)
(let pool_running true)

(log "=== Power Monitor ===")
(log "Total load:")
(log total_watts)

(let load_pct (* (/ total_watts limit_watts) 100))
(log "Load percentage:")
(log load_pct)

(if (>= load_pct 100)
    (begin
        (log "OVERLOAD — tripping main breaker!")
        (gate main_breaker off)
        (gate alert_siren on))
    (if (>= load_pct 80)
        (begin
            (log "Load is HIGH — shedding non-essential loads")
            (if (== dryer_running true)
                (begin
                    (log "Shedding: dryer")
                    (gate dryer off)))
            (if (== pool_running true)
                (begin
                    (log "Shedding: pool pump")
                    (gate pool_pump off)))
            (log "Non-essential loads shed"))
        (begin
            (log "Power draw normal")
            (gate alert_led off))))
