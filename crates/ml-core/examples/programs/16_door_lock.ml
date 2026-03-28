;; ============================================================
;; Smart Door Lock — keypad with brute-force protection
;; ============================================================
;; Locks after 3 wrong code attempts. Auto-locks at night.
;; User can unlock during the day with correct code.
;; Uses: let, if, set, while, gate, log, begin, binary ops
;;
;; Expected output (2 wrong attempts, then correct):
;;   [ML] === Door Lock ===
;;   [ML] Attempt 1: WRONG
;;   [ML] Attempts: 1
;;   [ML] Attempt 2: WRONG
;;   [ML] Attempts: 2
;;   [ML] Attempt 3: CORRECT
;;   [Mock] gate 'door_lock' -> unlock

(let attempt 1)
(let max_attempts 3)
(let locked true)
(let correct_code 1234)
(let entered_code 1234)
(let hour 14)

(log "=== Door Lock ===")
(log "Hour:")
(log hour)

;; Auto-lock at night
(if (>= hour 22)
    (begin
        (log "Nighttime — auto-locking")
        (gate door_lock lock)
        (set locked true))
    (begin
        (log "Daytime hours")
        (gate door_lock unlock)
        (set locked false)))

;; Simulate 2 wrong attempts then correct
(while (< attempt 3)
    (begin
        (log "Attempt ")
        (log attempt)
        (if (== entered_code correct_code)
            (begin
                (log "CORRECT")
                (gate door_lock unlock)
                (set locked false))
            (begin
                (log "WRONG")
                (set attempt (+ attempt 1))
                (if (>= attempt max_attempts)
                    (begin
                        (log "LOCKOUT — too many attempts")
                        (gate alarm on)))))))
