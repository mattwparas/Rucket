(define (collatz n)
    (if (= n 1) 
        '(1)
        (cons n (collatz (if (even? n) (/ n 2) (+ 1 (* 3 n)))))))

(define (collatz-length n)
    (define (aux n r)
        (if (= n 1) 
                r
                (aux (if (even? n) (/ n 2) (+ 1 (* 3 n))) (+ r 1))))
    (aux n 1))

; (define (collatz-max a b)
;     (define (aux i j k)
;         (if (> i b) 
;                 (list j k)
;                 (let ((h (collatz-length i)))
;                     (if (> h k) 
;                         (aux (+ i 1) i h) 
;                         (aux (+ i 1) j k)))))
;     (aux a 0 0))
        
(define (maux i j k b)
        (if (> i b) 
                (list j k)
                (let ((h (collatz-length i)))
                    (if (> h k) 
                        (maux (+ i 1) i h b) 
                        (maux (+ i 1) j k b)))))


(define (collatz-max a b)
    
    (maux a 0 0 b))

        
; (define (collatz-max a b)
;     (define (aux i j k b)
;         (if (> i b) 
;                 (list j k)
;                 (let ((h (collatz-length i)))
;                     (if (> h k) 
;                         (aux (+ i 1) i h b) 
;                         (aux (+ i 1) j k b)))))
;     (aux a 0 0 b))


; (displayln (collatz 27))
; (27 82 41 124 62 31 94 47 142 71 214 107 322 161 484 242 121 364 182
; 91 274 137 412 206 103 310 155 466 233 700 350 175 526 263 790 395
; 1186 593 1780 890 445 1336 668 334 167 502 251 754 377 1132 566 283
; 850 425 1276 638 319 958 479 1438 719 2158 1079 3238 1619 4858 2429
; 7288 3644 1822 911 2734 1367 4102 2051 6154 3077 9232 4616 2308 1154
; 577 1732 866 433 1300 650 325 976 488 244 122 61 184 92 46 23 70 35
; 106 53 160 80 40 20 10 5 16 8 4 2 1)

; (displayln (collatz-length 27))
; 112

(displayln (collatz-max 1 100000))
; (77031 351)