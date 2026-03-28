;; ============================================================
;; Lights Out Game — classic puzzle implemented in ML
;; ============================================================
;; A 3x3 grid of lights. Pressing a light toggles it and
;; adjacent lights. Goal: turn all lights off.
;; Uses: let, set, while, if, gate, log, begin, binary ops
;;
;; This version simulates 5 moves of an active game.
;; Expected output:
;;   [ML] === Lights Out! ===
;;   [ML] Grid: 5 lights on
;;   [ML] Move 1: toggle (1,1)
;;   [ML] Grid: 3 lights on
;;   [ML] Move 2: toggle (0,0)
;;   ...

(let lights_on 9)
(let move 0)
(let max_moves 5)

(log "=== Lights Out! ===")
(log "Starting grid:")
(log lights_on)

(while (< move max_moves)
    (begin
        (log "Move ")
        (log (+ move 1))
        (log ": toggle")
        (set lights_on (- lights_on 2))
        (if (> lights_on 0)
            (begin
                (log "Still ")
                (log lights_on)
                (log " lights on"))
            (begin
                (log "YOU WIN!")
                (gate victory_led on)))
        (set move (+ move 1))))
