;; destruct works like so:
;; (destruct (a b c) value)
;;  ...
;; (define a (car value))
;; (define b (car (cdr value)))
;; (define c (car (cdr (cdr value))))
(define-syntax destruct
(syntax-rules ()
    [(destruct (var) ret-value)
    (define (datum->syntax var) (car ret-value))]
    [(destruct (var1 var2 ...) ret-value)
    (begin (define (datum->syntax var1) (car ret-value))
            (destruct (var2 ...) (cdr ret-value)))]))


(define (any? v) #t)
(define stack? list?)
(define (pair? s)
(and (list? s) (= (length s) 2)))


(define (make-stack) '())

(define/contract (pop stack)
    (->/c stack? pair?)
    (if (null? stack)
        '(#f '())
        (list (car stack) (cdr stack))))

;; value -> stack -> stack
(define push cons)

;; instantiate an empty stack
(define my-stack (make-stack))

;; Bind the last few values from the stack
;; Push the values 1, 2, 3, 4, then pop and return the value and the stack
(destruct (pop-val new-stack)
        (->> my-stack
            (push 1)
            (push 2)
            (push 3)
            (push 4)
            (pop)))

pop-val ;; => 4
new-stack ;; => '(3 2 1)
(assert! (equal? pop-val 4))