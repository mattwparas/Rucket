; #lang racket/base

;;; The Computer Language Benchmarks Game
;;; https://salsa.debian.org/benchmarksgame-team/benchmarksgame/

;;; Derived from the Chicken variant by Sven Hartrumpf
;;; contributed by Matthew Flatt
;;; *reset*

; (require racket/cmdline)

(struct node (left val right))

;; Instead of (define-struct leaf (val)):
(define (leaf val) (node #f val #f))
(define (leaf? l) (not (node-left l)))
(define (leaf-val l) (node-val l))

(define (make item d)
  (if (= d 0)
      (leaf item)
      (let ((item2 (* item 2))
            (d2 (- d 1)))
        (node (make (- item2 1) d2) 
              item 
              (make item2 d2)))))

(define (check t)
  (if (leaf? t)
      1
      (+ 1 (+ (check (node-left t)) 
                         (check (node-right t))))))

(define (main n)
  (let* ((min-depth 4)
         (max-depth (max (+ min-depth 2) n)))
    (let ((stretch-depth (+ max-depth 1)))
      (begin
        (display "stretch tree of depth ")
        (display stretch-depth)
        (display " check: ")
        (displayln (check (make 0 stretch-depth)))))
    (let ((long-lived-tree (make 0 max-depth)))
      (begin
        (define end (add1 max-depth))
        (define (loop d)
          (if (>= d end)
              void
              (begin
                (let ((iterations (arithmetic-shift 1 (+ (- max-depth d) min-depth))))
                  (display iterations)
                  (display " trees of depth ")
                  (display d)
                  (display " check: ")
                  (displayln
                    (transduce 
                      (mapping (lambda (i) (make i d)))
                      (lambda (c i) (+ c (check i)))
                      0
                      (range 0 iterations))))
                (loop (+ 2 d)))))
        (loop 4))

      
      (begin (display "long lived tree of depth ")
             (display max-depth)
             (display "check: ")
             (displayln (check long-lived-tree))))))

; (main 21)


; (command-line #:args (n) 
;               (main (string->number n)))
