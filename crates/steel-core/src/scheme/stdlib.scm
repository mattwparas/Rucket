(define-syntax steel/base
  (syntax-rules ()
    [(steel/base) (require-builtin steel/base)]))

; (define-syntax quasiquote
;   (syntax-rules (unquote unquote-splicing)
;     ((_ ((unquote x) . xs))          (cons x (quasiquote xs)))
;     ((_ ((unquote-splicing x) . xs)) (append x (quasiquote xs)))
;     ((_ (unquote x))                 x)
;     ((_ (x  . xs))                   (cons (quasiquote x) (quasiquote xs)))
;     ((_ x)                           (quote x))))

(define-syntax quasiquote
  (syntax-rules (unquote unquote-splicing #%unquote #%unquote-splicing #%quote)

    [(quasiquote ((quote x) xs ...)) (cons (list 'quote (quasiquote x)) (quasiquote (xs ...)))]

    [(quasiquote (quote x)) (list 'quote (quasiquote x))]

    [(quasiquote ((unquote x) xs ...)) (cons (list 'unquote (quasiquote x)) (quasiquote (xs ...)))]
    [(quasiquote (unquote x)) (list 'unquote (quasiquote x))]

    ; ((quasiquote ((#%unquote x) xs ...))          (cons x (quasiquote (xs ...))))

    [(quasiquote ((#%unquote x) xs ...)) (cons x (quasiquote (xs ...)))]
    [(quasiquote (#%unquote x)) x]

    [(quasiquote ((#%unquote-splicing x))) (append x '())]
    [(quasiquote ((#%unquote-splicing x) xs ...)) (append x (quasiquote (xs ...)))]

    ;; TODO: Do unquote-splicing as well, follow the same rules as unquote
    [(quasiquote ((unquote-splicing x))) (append (list (list 'unquote-splicing (quasiquote x))) '())]
    [(quasiquote ((unquote-splicing x) xs ...))
     (append (list (list 'unquote-splicing (quasiquote x))) (quasiquote (xs ...)))]
    [(quasiquote (x xs ...)) (cons (quasiquote x) (quasiquote (xs ...)))]
    [(quasiquote x) 'x]))

(define-syntax #%proto-syntax-object
  (syntax-rules ()
    [(#%proto-syntax-object x) (#%syntax/raw 'x 'x (#%syntax-span x))]))

;; TODO: @Matt
;; Bootstrap this by expanding first, then expanding the resulting
;; TODO: Add syntax->span in the macro expander as a special case function.
;; That just inlines the span object '(left right <source-id>)
;; And then also calls the constructor for the
(define-syntax quasisyntax
  (syntax-rules (syntax unsyntax unsyntax-splicing #%unsyntax #%unsyntax-splicing #%internal-crunch)

    [(quasisyntax ((syntax x) xs ...)) (cons (list 'syntax (quasisyntax x)) (quasisyntax (xs ...)))]

    [(quasisyntax (syntax x)) (list 'quote (quasisyntax x))]

    [(quasisyntax ((unsyntax x) xs ...))
     (cons (list 'unsyntax (quasisyntax x)) (quasisyntax (xs ...)))]
    [(quasisyntax (unsyntax x)) (list 'unsyntax (quasisyntax x))]

    ; ((quasiquote ((#%unquote x) xs ...))          (cons x (quasiquote (xs ...))))

    [(quasisyntax ((#%unsyntax x) xs ...)) (cons x (quasisyntax (xs ...)))]
    [(quasisyntax (#%unsyntax x)) x]

    [(quasisyntax ((#%unsyntax-splicing x))) (append x '())]
    [(quasisyntax ((#%unsyntax-splicing x) xs ...)) (append x (quasisyntax (xs ...)))]

    ;; TODO: Do unquote-splicing as well, follow the same rules as unquote
    [(quasisyntax ((unsyntax-splicing x)))
     (append (list (list 'unsyntax-splicing (quasisyntax x))) '())]
    [(quasisyntax ((unsyntax-splicing x) xs ...))
     (append (list (list 'unsyntax-splicing (quasisyntax x))) (quasisyntax (xs ...)))]

    [(quasisyntax #%internal-crunch ()) (list)]
    ; (list
    ; (#%syntax/raw
    ;   (quote x)

    ;   (cons (quasisyntax x) (quasisyntax #%internal-crunch (xs ...)))

    ;   (#%syntax-span x)))

    [(quasisyntax #%internal-crunch (x xs ...))
     ; (list
     ; (#%syntax/raw
     ; (quote x)

     (cons (quasisyntax x) (quasisyntax #%internal-crunch (xs ...)))]

    ; (#%syntax-span x)))

    [(quasisyntax (x xs ...))
     ; (list
     (#%syntax/raw (quote (x xs ...))
                   (cons (quasisyntax x) (quasisyntax #%internal-crunch (xs ...)))
                   (#%syntax-span (x xs ...)))]
    ; )

    ; ((quasisyntax x)                          'x)))

    ; ((quasisyntax (quote ()))     '())
    [(quasisyntax x) (if (empty? 'x) (list) (#%syntax/raw 'x 'x (#%syntax-span x)))]))

(define-syntax or
  (syntax-rules ()
    [(or) #f]
    [(or x) x]
    [(or x y) (let ([z x]) (if z z y))]
    [(or x y ...) (or x (or y ...))]))

(define-syntax and
  (syntax-rules ()
    [(and) #t]
    [(and x) x]
    [(and x y) (if x y #f)]
    [(and x y ...) (and x (and y ...))]))

(define-syntax when
  (syntax-rules ()
    [(when a
       b ...)
     (if a
         (begin
           b ...)
         void)]))

(define-syntax unless
  (syntax-rules ()
    [(unless a
       b ...)
     (if a
         void
         (begin
           b ...))]))

(define-syntax cond
  (syntax-rules (else =>)
    [(cond
       [else
        =>
        e1 ...])
     (begin
       e1 ...)]
    [(cond
       [else
        e1 ...])
     (begin
       e1 ...)]
    [(cond
       [e1
        e2 ...])
     (when e1
       e2 ...)]
    [(cond
       [e1
        =>
        e2 ...]
       c1 ...)
     (if e1
         (begin
           e2 ...)
         (cond
           c1 ...))]
    [(cond
       [e1
        e2 ...]
       c1 ...)
     (if e1
         (begin
           e2 ...)
         (cond
           c1 ...))]))

(define-syntax case
  (syntax-rules (else)
    [(case (key ...)
       clauses ...)
     (let ([atom-key (key ...)])
       (case atom-key
         clauses ...))]
    [(case key
       [else
        result1
        result2 ...])
     (begin
       result1
       result2 ...)]
    [(case key
       [(atoms ...)
        result1
        result2 ...])
     (when (member key '(atoms ...))
       (begin
         result1
         result2 ...))]

    ; [(case key
    ;        ((atoms ...) result1 ...)
    ;        clause clauses ...)
    ;      (if (member key '(atoms ...))
    ;          (begin result1 ...)
    ;          (case key clause clauses ...))]

    [(case key
       [(atoms ...)
        result1
        result2 ...]
       clause
       clauses ...)
     (if (member key '(atoms ...))
         (begin
           result1
           result2 ...)
         (case key
           clause
           clauses ...))]))

(define-syntax while
  (syntax-rules (do)
    [(while cond do body ...)
     (begin
       (define (loop)
         (when cond
           body ...
           (loop)))
       (loop))]
    [(while cond body ...)
     (begin
       (define (loop)
         (when cond
           body ...
           (loop)))
       (loop))]))

;; TODO add the single argument case
(define-syntax f>
  (syntax-rules ()
    [(f> fun) fun]
    [(f> fun args* ...) (lambda (x) (fun x args* ...))]))

(define-syntax ->
  (syntax-rules ()
    [(-> a) a]
    [(-> a (b c ...)) ((f> b c ...) a)]
    [(-> a (b)) ((f> b) a)]
    [(-> a b c ...) (-> (-> a b) c ...)]))

(define-syntax ~>
  (syntax-rules ()
    [(~> a) a]
    [(~> a (b c ...)) ((f> b c ...) a)]
    [(~> a (b)) ((f> b) a)]
    [(~> a b) ((f> b) a)]
    [(~> a b c ...) (~> (~> a b) c ...)]))

(define-syntax l>
  (syntax-rules ()
    [(l> fun) fun]
    [(l> fun args* ...) (lambda (x) (fun args* ... x))]))

(define-syntax ~>>
  (syntax-rules ()
    [(~>> a) a]
    [(~>> a (b c ...)) ((l> b c ...) a)]
    [(~>> a (b)) ((l> b) a)]
    [(~>> a b) ((l> b) a)]
    [(~>> a b c ...) (~>> (~>> a b) c ...)]))

(define-syntax ->>
  (syntax-rules ()
    [(->> a) a]
    [(->> a (b c ...)) ((l> b c ...) a)]
    [(->> a (b)) ((l> b) a)]
    [(->> a b c ...) (->> (->> a b) c ...)]))

(define-syntax swap
  (syntax-rules ()
    [(swap a b)
     (let ([tmp b])
       (begin
         (set! b a)
         (set! a tmp)))]))

(define-syntax let*
  (syntax-rules ()
    [(let* ()
       body ...) ; base case
     ((lambda ()
        body ...))]
    [(let* ([var val] rest ...)
       body ...) ; binding case
     ((lambda (var)
        (let* (rest ...)
          body ...))
      val)]))

(define-syntax letrec*-helper
  (syntax-rules ()
    [(letrec*-helper () body ...)
     (begin
       body ...)]
    [(letrec*-helper ((var val) rest ...) body ...)
     (begin
       (define var val)
       (letrec*-helper (rest ...) body ...))]))

(define-syntax letrec*
  (syntax-rules ()
    [(letrec* bindings body ...) ((lambda () (letrec*-helper bindings body ...)))]))

(define-syntax module
  (syntax-rules (provide gen-defines
                         contract/out)
    [(module name (provide ids ...)
       funcs ...)
     (begin
       (define (datum->syntax name)
         ((lambda ()
            funcs ...
            (module provide ids
              ...))))
       (module gen-defines name
         ids ...))]

    ;; in the contract case, ignore the contract in the hash
    [(module provide (contract/out name contract)
       )
     (%proto-hash% 'name name)]
    ;; Normal case
    [(module provide name
       )
     (%proto-hash% 'name name)]

    ;; in the contract case, ignore the contract in the hash
    [(module provide (contract/out name contract)
       rest ...)
     (%proto-hash-insert% (module provide rest
                            ...)
                          'name
                          name)]

    ;; Normal case
    [(module provide name
       rest ...)
     (%proto-hash-insert% (module provide rest
                            ...)
                          'name
                          name)]

    ;; Module contract provides
    [(module gen-defines mod
       (contract/out name contract))
     (define (datum->syntax name)
       (bind/c contract (%proto-hash-get% mod 'name)))]
    [(module gen-defines mod
       (contract/out name contract)
       rest ...)
     (begin
       (define (datum->syntax name)
         (bind/c contract (%proto-hash-get% mod 'name)))
       (module gen-defines mod
         rest ...))]

    ;; Normal provides
    [(module gen-defines mod
       name)
     (define (datum->syntax name)
       (%proto-hash-get% mod 'name))]
    [(module gen-defines mod
       name
       rest ...)
     (begin
       (define (datum->syntax name)
         (%proto-hash-get% mod 'name))
       (module gen-defines mod
         rest ...))]))

;; TODO: Replace some of these with just list ref to abuse the underlying implementation
(define caar (lambda (pair) (car (car pair))))
(define cadr (lambda (pair) (car (cdr pair))))
(define cdar (lambda (pair) (cdr (car pair))))
(define cddr (lambda (pair) (cdr (cdr pair))))
(define caaar (lambda (pair) (car (car (car pair)))))
(define caadr (lambda (pair) (car (car (cdr pair)))))
(define cadar (lambda (pair) (car (cdr (car pair)))))
(define caddr (lambda (pair) (car (cdr (cdr pair)))))
(define cdaar (lambda (pair) (cdr (car (car pair)))))
(define cdadr (lambda (pair) (cdr (car (cdr pair)))))
(define cddar (lambda (pair) (cdr (cdr (car pair)))))
(define cdddr (lambda (pair) (cdr (cdr (cdr pair)))))
(define caaaar (lambda (pair) (car (car (car (car pair))))))
(define caaadr (lambda (pair) (car (car (car (cdr pair))))))
(define caadar (lambda (pair) (car (car (cdr (car pair))))))
(define caaddr (lambda (pair) (car (car (cdr (cdr pair))))))
(define cadaar (lambda (pair) (car (cdr (car (car pair))))))
(define cadadr (lambda (pair) (car (cdr (car (cdr pair))))))
(define caddar (lambda (pair) (car (cdr (cdr (car pair))))))
(define cadddr (lambda (pair) (car (cdr (cdr (cdr pair))))))
(define cdaaar (lambda (pair) (cdr (car (car (car pair))))))
(define cdaadr (lambda (pair) (cdr (car (car (cdr pair))))))
(define cdadar (lambda (pair) (cdr (car (cdr (car pair))))))
(define cdaddr (lambda (pair) (cdr (car (cdr (cdr pair))))))
(define cddaar (lambda (pair) (cdr (cdr (car (car pair))))))
(define cddadr (lambda (pair) (cdr (cdr (car (cdr pair))))))
(define cdddar (lambda (pair) (cdr (cdr (cdr (car pair))))))
(define cddddr (lambda (pair) (cdr (cdr (cdr (cdr pair))))))
(define id (lambda (obj) obj))
(define flip (lambda (func) (lambda (arg1 arg2) (func arg2 arg1))))
(define curry (lambda (func arg1) (lambda (arg) (func arg1 arg))))
(define curry2 (lambda (func arg1) (lambda (arg2 arg3) (func arg1 arg2 arg3))))
; (define compose (lambda (f g) (lambda (arg) (f (g arg)))))

(define (not a)
  (if a #f #t))

(define (foldl func accum lst)
  (if (null? lst)
      accum
      (foldl func
             (func (car lst) accum) ; here's the change
             (cdr lst))))

(define (map func lst . lsts)

  (cond
    [(null? lst) '()]
    [(null? lsts) (transduce lst (mapping func) (into-list))]
    [else
     =>
     (define (crunch composer remaining-lists)
       (if (null? remaining-lists)
           composer
           (crunch (compose composer (zipping (car remaining-lists))) (cdr remaining-lists))))
     (if (null? lsts)
         (map func lst)
         ;; Handle the case for many lists
         (let ([composed-transducer (crunch (compose) lsts)])
           (transduce lst composed-transducer (mapping (lambda (x) (apply func x))) (into-list))))]))

; (if (null? lst)
;     '()
;     (transduce lst (mapping func) (into-list))))

(define foldr
  (lambda (func accum lst) (if (null? lst) accum (func (car lst) (foldr func accum (cdr lst))))))

(define unfold
  (lambda (func init pred)
    (if (pred init) (cons init '()) (cons init (unfold func (func init) pred)))))

(define fold (lambda (f a l) (foldl f a l)))
(define reduce (lambda (f a l) (fold f a l)))
(define max (lambda (x num-list) (fold (lambda (y z) (if (> y z) y z)) x (cons 0 num-list))))
(define min (lambda (x num-list) (fold (lambda (y z) (if (< y z) y z)) x (cons 536870911 num-list))))

(define mem-helper
  (lambda (pred op) (lambda (acc next) (if (and (not acc) (pred (op next))) next acc))))
;; (define memq (lambda (obj lst)       (fold (mem-helper (curry eq? obj) id) #f lst)))
; (define memv (lambda (obj lst)       (fold (mem-helper (curry eqv? obj) id) #f lst)))
; (define member (lambda (obj lst)     (fold (mem-helper (curry equal? obj) id) #f lst)))

(define member
  (lambda (x los)
    (cond
      [(null? los) #f]
      [(equal? x (car los)) los]
      [else (member x (cdr los))])))

(define (contains? pred? lst)
  ; (displayln lst)
  (cond
    [(empty? lst) #f]
    [(pred? (car lst)) #t]
    [else (contains? pred? (cdr lst))]))

;; TODO come back to this
; (define assq (lambda (obj alist)     (fold (mem-helper (curry eq? obj) car) #f alist)))

;; (define assv (lambda (obj alist)     (fold (mem-helper (curry eqv? obj) car) #f alist)))
; (define assoc (lambda (obj alist)    (fold (mem-helper (curry equal? obj) car) #f alist)))

; (define assoc )

(define (assoc thing alist)
  (if (null? alist) #f (if (equal? (car (car alist)) thing) (car alist) (assoc thing (cdr alist)))))

(define (filter pred lst)
  (if (empty? lst) '() (transduce lst (filtering pred) (into-list))))

; (define (fact n)
;   (define factorial-tail (lambda (n acc)
;                            (if (= n 0)
;                                acc
;                                (factorial-tail (- n 1)  (* acc n )))))
;   (factorial-tail n 1))

(define even-rec? (lambda (x) (if (= x 0) #t (odd-rec? (- x 1)))))
(define odd-rec? (lambda (x) (if (= x 0) #f (even-rec? (- x 1)))))

(define sum (lambda (x) (reduce + 0 x)))
;; (define head car)
;; (define tail cdr)
(define (add1 n)
  (+ 1 n))
(define (sub1 n)
  (- n 1))
(define (zero? n)
  (= n 0))

;; currently broken, doesn't work properly
; (defn (take lst n)
;   (defn (loop x l acc)
;     (if (= x 0)
;         acc
;         (loop (- x 1) (cdr l) (cons (car l) acc))))
;   (loop n lst (list)))

(define (drop lst n)
  (define (loop x l)
    (if (zero? x) l (loop (sub1 x) (cdr l))))
  (loop n lst))

(define (slice l offset n)
  (take (drop l offset) n))

(define (flatten lst)
  (cond
    [(null? lst) '()]
    [(list? lst) (append (flatten (car lst)) (flatten (cdr lst)))]
    [else (list lst)]))

;;; Macros go here:

(define-syntax reset
  (syntax-rules ()
    [(reset ?e) (*reset (lambda () ?e))]))

(define-syntax shift
  (syntax-rules ()
    [(shift ?k ?e) (*shift (lambda (?k) ?e))]))

;; TODO: This should be boxed at some point, we don't want it
;; to be globally accessible directly (I think)
(define (*meta-continuation* v)
  (error "You forgot the top-level reset..."))

(define (*abort thunk)
  (let ([v (thunk)]) (*meta-continuation* v)))

(define (*reset thunk)
  (let ([mc *meta-continuation*])
    (call/cc (lambda (k)
               (begin
                 (set! *meta-continuation*
                       (lambda (v)
                         (set! *meta-continuation* mc)
                         (k v)))
                 (*abort thunk))))))

(define (*shift f)
  (call/cc (lambda (k) (*abort (lambda () (f (lambda (v) (reset (k v)))))))))

(define-syntax with-handler
  (syntax-rules ()
    [(with-handler handler expr)
     (reset (call-with-exception-handler (lambda (err)
                                           (handler err)
                                           (shift k (k void)))
                                         (lambda () expr)))]
    [(with-handler handler expr ...)
     (reset (call-with-exception-handler (lambda (err)
                                           (handler err)
                                           (shift k (k void)))
                                         (lambda ()
                                           expr ...)))]))

(define-syntax case-lambda
  (syntax-rules ()
    [(case-lambda) (lambda args (error "CASE-LAMBDA without any clauses."))]
    [(case-lambda
       [?a1
        ?e1 ...]
       ?clause1 ...)
     (lambda args
       (%plain-let ((l (length args)))
                   (case-lambda
                     "CLAUSE"
                     args
                     l
                     [?a1
                      ?e1 ...]
                     ?clause1 ...)))]
    [(case-lambda
       "CLAUSE"
       ?args
       ?l
       [(?a1 ...)
        ?e1 ...])
     (if (= ?l (length '(?a1 ...)))
         (apply (lambda (?a1 ...)
                  ?e1 ...)
                ?args)
         (error! "Arity mismatch"))]
    [(case-lambda
       "CLAUSE"
       ?args
       ?l
       [(?a1 ...)
        ?e1 ...]
       ?clause1 ...)
     (if (= ?l (length '(?a1 ...)))
         (apply (lambda (?a1 ...)
                  ?e1 ...)
                ?args)
         (case-lambda
           "CLAUSE"
           ?args
           ?l
           ?clause1 ...))]
    [(case-lambda
       "CLAUSE"
       ?args
       ?l
       [(?a1 . ?ar)
        ?e1 ...]
       ?clause1 ...)
     (case-lambda
       "IMPROPER"
       ?args
       ?l
       1
       [?a1
        . ?ar]
       [?ar
        ?e1 ...]
       ?clause1 ...)]
    [(case-lambda
       "CLAUSE"
       ?args
       ?l
       [?a1
        ?e1 ...])
     (%plain-let ((?a1 ?args)) ?e1 ...)]
    [(case-lambda
       "CLAUSE"
       ?args
       ?l)
     (error "Wrong number of arguments to CASE-LAMBDA.")]
    [(case-lambda
       "IMPROPER"
       ?args
       ?l
       ?k
       ?al
       [(?a1 . ?ar)
        ?e1 ...]
       ?clause1 ...)
     (case-lambda
       "IMPROPER"
       ?args
       ?l
       [+
        ?k
        1]
       ?al
       [?ar
        ?e1 ...]
       ?clause1 ...)]
    [(case-lambda
       "IMPROPER"
       ?args
       ?l
       ?k
       ?al
       [?ar
        ?e1 ...]
       ?clause1 ...)
     (if (>= ?l ?k)
         (apply (lambda ?al
                  ?e1 ...)
                ?args)
         (case-lambda
           "CLAUSE"
           ?args
           ?l
           ?clause1 ...))]))

(define-syntax help
  (syntax-rules ()
    [(help) (simple-displayln "help expects an identifier to lookup documentation for")]
    [(help ident) (%doc? %-builtin-module-steel/base (quote ident))]
    [(help module ident) (%doc? (datum->syntax %-builtin-module- module) (quote ident))]))

(define-syntax dbg!
  (syntax-rules ()
    [(dbg! expr)
     (let ([result expr])
       (simple-displayln (quote expr) " = " result)
       result)]))

(define-syntax contract/out
  (syntax-rules ()
    [(contract/out name contract) (%require-ident-spec name (bind/c contract name 'name))]))

(define (force promise)
  (promise))

;; syntax
(define-syntax delay
  (syntax-rules ()
    [(delay expr) (lambda () expr)]))

(define values list)
(define (call-with-values producer consumer)
  (apply consumer (producer)))
