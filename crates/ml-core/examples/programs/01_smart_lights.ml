;; ============================================================
;; Smart Lights — time-based outdoor lighting automation
;; ============================================================
;; Turns courtyard lights on at sunset (hour >= 18),
;; keeps them on until midnight (hour >= 0), then off.
;; Uses: let, if, gate, log, begin
;;
;; Expected output (time=20):
;;   [ML] Lights on for the night
;;   [Mock] gate 'courtyard_lights' -> on
;;
;; Run: cargo run -p ml-core --example run -- "$(cat examples/programs/01_smart_lights.ml | grep -v '^;;')"

(let lights_on false)
(let time 20)   ;; 8 PM — simulate evening

(if (> time 18)
    (begin
        (log "Lights on for the night")
        (gate courtyard_lights on))
    (begin
        (log "Daytime — lights off")
        (gate courtyard_lights off)))
