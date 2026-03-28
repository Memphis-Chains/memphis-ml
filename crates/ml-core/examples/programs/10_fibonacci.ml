;; ============================================================
;; Fibonacci — iterative computation using while loop
;; ============================================================
;; Computes the Nth Fibonacci number iteratively.
;; F(0)=0, F(1)=1, F(n)=F(n-1)+F(n-2)
;; Uses: let, set, while, if, log, begin, binary ops
;;
;; Expected output (n=10):
;;   [ML] Fibonacci sequence up to position 10:
;;   [ML] 0
;;   [ML] 1
;;   [ML] 1
;;   [ML] 2
;;   ... (up to 55)

(let n 10)
(let a 0)
(let b 1)
(let count 0)

(log "Fibonacci sequence up to position ")
(log n)

(while (< count n)
    (begin
        (log a)
        (let next (+ a b))
        (set a b)
        (set b next)
        (set count (+ count 1))))

(log "Done!")
