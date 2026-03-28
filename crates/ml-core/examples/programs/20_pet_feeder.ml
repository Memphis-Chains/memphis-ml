;; ============================================================
;; Pet Feeder — automated feeding with portion control
;; ============================================================
;; Dispenses food at scheduled times (7 AM, 6 PM).
;; Tracks daily portions and blocks over-feeding.
;; Uses: let, if, gate, log, begin, binary ops
;;
;; Expected output (7 AM, portion 1 of 2):
;;   [ML] === Pet Feeder ===
;;   [ML] Breakfast time — portion 1
;;   [Mock] gate 'food_dispenser' -> dispense
;;   [ML] Daily portions: 1
;;   [ML] Slow release — food bowl empty

(let hour 7)
(let daily_portions 0)
(let max_daily_portions 2)
(let portion_size 1)    ;; 1 = normal, 0.5 = half
(let cat_hungry true)

(log "=== Pet Feeder ===")
(log "Hour:")
(log hour)
(log "Daily portions so far:")
(log daily_portions)

(if (>= hour 6)
    (if (< hour 8)
        (begin
            (log "Breakfast time")
            (if (< daily_portions max_daily_portions)
                (begin
                    (log "Dispensing breakfast portion")
                    (gate food_dispenser dispense)
                    (set daily_portions (+ daily_portions 1))
                    (log daily_portions)
                    (log "Daily portions")
                    (if (== portion_size 0.5)
                        (log "Half portion for weight management")
                    (log "Normal portion"))
                    (if (> daily_portions max_daily_portions)
                        (begin
                            (log "Daily limit reached — no more food")
                            (gate feeder_lockout on))))
            (begin
                (log "Already fed max portions today")
                (gate feeder_lockout on))))
        (if (>= hour 17)
            (if (< hour 19)
                (begin
                    (log "Dinner time")
                    (if (< daily_portions max_daily_portions)
                        (begin
                            (log "Dispensing dinner portion")
                            (gate food_dispenser dispense)
                            (set daily_portions (+ daily_portions 1))
                            (log daily_portions)
                            (log "Daily portions total"))
            (begin
                (log "No more feedings scheduled")))
        (begin
            (log "Not a scheduled feeding time")
            (if (== cat_hungry true)
                (log "Cat is hungry but not scheduled — no food")
            (log "Cat not hungry"))))))
