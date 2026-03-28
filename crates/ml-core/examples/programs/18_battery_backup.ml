;; ============================================================
;; Battery Backup (UPS) — failover and recovery management
;; ============================================================
;; Monitors mains power. On failure: switch to battery + generator.
;; On recovery: recharge battery before switching back.
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (on battery, charge=35, recovering):
;;   [ML] === Battery Backup System ===
;;   [ML] Mains FAILED — on battery
;;   [ML] Battery: 35%
;;   [ML] Battery LOW — starting generator
;;   [Mock] gate 'generator' -> start
;;   [Mock] gate 'inverter' -> on

(let mains_power false)
(let battery_charge 35)
(let generator_running false)
(let mains_recovery false)

(log "=== Battery Backup System ===")
(log "Mains power:")
(log mains_power)
(log "Battery charge:")
(log battery_charge)

(if (== mains_power true)
    (begin
        (log "Mains power OK")
        (gate inverter off)
        (gate generator off)
        (gate mains_contactor on)
        (if (> battery_charge 90)
            (begin
                (log "Battery full — switching to grid")
                (gate charger off))
            (begin
                (log "Recharging battery from grid")
                (gate charger on))))
    (begin
        (log "Mains FAILED — on battery")
        (gate mains_contactor off)
        (gate inverter on)
        (if (< battery_charge 20)
            (begin
                (log "Battery LOW — starting generator")
                (gate generator start)
                (set generator_running true))
            (begin
                (log "Battery OK — running off battery")
                (gate charger off))))))

(if (== mains_recovery true)
    (begin
        (log "Mains recovering — synchronizing")
        (gate generator off)
        (gate mains_contactor on)
        (gate charger on)
        (log "Switched back to mains")))
