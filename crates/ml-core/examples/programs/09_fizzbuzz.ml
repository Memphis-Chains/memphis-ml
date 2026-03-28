;; ============================================================
;; FizzBuzz — classic programming challenge in ML
;; ============================================================
;; For numbers 1 to 20:
;;   - "FizzBuzz" if divisible by 3 and 5
;;   - "Fizz" if divisible by 3
;;   - "Buzz" if divisible by 5
;;   - the number otherwise
;; Uses: let, set, while, if, log, begin, binary ops, %
;;
;; Expected output: 1, 2, Fizz, 4, Buzz, Fizz, 7, 8, Fizz, Buzz,
;;                 11, Fizz, 13, 14, FizzBuzz, 16, 17, Fizz, 19, Buzz

(let n 1)
(let limit 20)

(while (<= n limit)
    (begin
        (if (== (% n 15) 0)
            (log "FizzBuzz")
            (if (== (% n 3) 0)
                (log "Fizz")
                (if (== (% n 5) 0)
                    (log "Buzz")
                    (log n))))
        (set n (+ n 1))))
