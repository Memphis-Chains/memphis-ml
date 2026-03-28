;; ============================================================
;; Security System — motion sensors + alarm + notifications
;; ============================================================
;; Armed mode: any motion triggers alarm.
;; Disarmed mode: no alarm regardless of motion.
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (armed, motion detected):
;;   [ML] === SECURITY: ARMED ===
;;   [ML] Motion detected in entryway!
;;   [Mock] gate 'siren' -> on
;;   [Mock] gate 'strobe' -> on
;;   [Mock] gate 'door_lock' -> on

(let armed true)
(let motion_detected true)
(let entryway_motion true)
(let door_closed true)

(log "=== Security System ===")

(if (== armed true)
    (begin
        (log "SECURITY: ARMED")
        (if (== motion_detected true)
            (begin
                (log "Motion detected!")
                (if (== entryway_motion true)
                    (begin
                        (log "Motion detected in entryway!")
                        (gate siren on)
                        (gate strobe on)
                        (gate door_lock on)))
                (gate alert_led on)))
        (if (== door_closed false)
            (begin
                (log "Door forced open!")
                (gate perimeter_alarm on))))
    (begin
        (log "System disarmed")
        (gate siren off)
        (gate strobe off)
        (gate door_lock off)))
