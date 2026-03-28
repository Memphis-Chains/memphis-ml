;; ============================================================
;; Pool Control — pump scheduling and chemistry automation
;; ============================================================
;; Runs pool pump in morning (6-9 AM) and evening (6-9 PM).
;; Monitors pH and chlorine levels; adds chemicals if needed.
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (7 AM, pH OK, low chlorine):
;;   [ML] === Pool Control ===
;;   [ML] Pump schedule: ACTIVE (morning window)
;;   [Mock] gate 'pool_pump' -> on
;;   [ML] pH OK — skipping acid
;;   [ML] Low chlorine — activating chlorinator
;;   [Mock] gate 'chlorinator' -> on

(let hour 7)
(let pH 7.4)
(let chlorine 2.1)
(let pump_running false)

(log "=== Pool Control ===")
(log "Pump schedule:")

(if (>= hour 6)
    (if (< hour 9)
        (begin
            (log "ACTIVE (morning window)")
            (gate pool_pump on)
            (set pump_running true))
        (if (>= hour 18)
            (if (< hour 21)
                (begin
                    (log "ACTIVE (evening window)")
                    (gate pool_pump on)
                    (set pump_running true))
            (begin
                (log "Inactive (outside schedule)")
                (gate pool_pump off)
                (set pump_running false))))
    (begin
        (log "Inactive (outside schedule)")
        (gate pool_pump off)
        (set pump_running false))))

(if (== pump_running true)
    (begin
        (log "Pool pump running — checking chemistry")
        (if (< pH 7.2)
            (begin
                (log "pH LOW — adding acid")
                (gate acid_feeder on))
            (if (> pH 7.6)
                (begin
                    (log "pH HIGH — adding base")
                    (gate base_feeder on))
                (begin
                    (log "pH OK — skipping acid")
                    (gate acid_feeder off))))
        (if (< chlorine 3.0)
            (begin
                (log "Low chlorine — activating chlorinator")
                (gate chlorinator on))
            (begin
                (log "Chlorine OK")
                (gate chlorinator off))))))
