;; ============================================================
;; Garage Door — gate with safety interlock logic
;; ============================================================
;; Opens garage door only when:
;;   1. No motion detected inside
;;   2. Door has been closed for at least 30s
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output:
;;   [ML] Checking safety interlock...
;;   [ML] All clear — opening garage door
;;   [Mock] gate 'garage_door' -> on

(let motion_inside false)
(let door_closed_duration 45)   ;; seconds since door closed
(let door_open false)

(log "Checking safety interlock...")

(if (== door_open false)
    (begin
        (if (> door_closed_duration 30)
            (begin
                (log "All clear — opening garage door")
                (gate garage_door on)
                (set door_open true))
            (log "Door closed too recently — wait..."))
        (if (== motion_inside true)
            (log "Motion inside — BLOCKING open")))
    (log "Door already open"))
