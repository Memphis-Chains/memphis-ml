;; ============================================================
;; Chicken Coop — automated door + supplemental lighting
;; ============================================================
;; Door opens at sunrise (6 AM) and closes at sunset (9 PM).
;; Light turns on in winter when daylight < 10h to boost egg production.
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (7 AM, winter mode):
;;   [ML] === Chicken Coop Automation ===
;;   [ML] Time: 7
;;   [ML] Opening coop door
;;   [Mock] gate 'coop_door' -> open
;;   [ML] Winter mode — supplemental light needed
;;   [Mock] gate 'coop_light' -> on

(let hour 7)
(let season winter)      ;; or: summer
(let daylight_hours 9)

(log "=== Chicken Coop Automation ===")
(log "Time:")
(log hour)

;; Door automation
(if (>= hour 6)
    (if (< hour 21)
        (begin
            (log "Opening coop door")
            (gate coop_door open))
        (begin
            (log "Closing coop door — nighttime")
            (gate coop_door close)))
    (begin
        (log "Door closed — too early")
        (gate coop_door close)))

;; Winter supplemental lighting (winter = less than 10h daylight)
(if (== season winter)
    (begin
        (log "Winter mode")
        (if (< daylight_hours 10)
            (begin
                (log "Supplemental light needed for egg production")
                (gate coop_light on))
            (begin
                (log "Enough daylight")
                (gate coop_light off))))
    (begin
        (log "Summer mode — no supplemental light needed")
        (gate coop_light off)))
