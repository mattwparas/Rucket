;; Enumerate the given list starting at index `start`
(define (enumerate start accum lst)
  (if (empty? lst)
      (reverse accum)
      (enumerate (+ start 1)
                 (cons (list (car lst) start)
                       accum)
                 (cdr lst))))

(define (hash->list hm)
  (transduce (transduce hm (into-list))
             (mapping (lambda (pair)
                        ;; If we have a symbol as a key, that means we need to quote it before
                        ;; we put it back into the map
                        (if (symbol? (car pair))
                            ;; TODO: @Matt - this causes a parser error
                            ;; (cons `(quote ,(car x)) (cdr x))
                            (cons (list 'quote (car pair)) (cdr pair))
                            pair)))
             (flattening)
             (into-list)))


;; ------------ Structs ---------------
;; Mutable structs in Steel are just implemented as fixed-size vectors, but with a little bit of magic.
;; The model is as follows:
;; (mutable-vector ___magic_struct_symbol___ 'struct-name fields ...)
;;
;; TODO: There is also room to implement traits in the struct field. Traits _themselves_ can be implemented as structs, and trait
;; resolution can be done with an index lookup on the struct with some layers of indirection.

;; Defines the predicate with the form `struct-name?`
(define (make-predicate struct-name fields)
  `(define ,(concat-symbols struct-name '?)
     (bind/c (make-function/c (make/c any/c 'any/c) (make/c boolean? 'boolean?))
             (lambda (this) (if (mutable-vector? this)
                                (if (eq? (mut-vector-ref this 0) ___magic_struct_symbol___)
                                    (equal? (mut-vector-ref this 1) (quote ,struct-name))
                                    #f)
                                #f))
             (quote ,(concat-symbols struct-name '?)))))

;; Defines the constructor with the form `struct-name`
;; There is room here for a lot more custom fields to increase the functionality
;; of structs. The first one would be the custom printing method.
;; 
;; TODO: lift options out of the struct itself into a singleton. Every constructor doesn't need
;; to have to reconstruct the options map on each instance. Probably some sort of
;; internal vtable would be a good place to store it, or just make some sort of namespaced
;; options that the later instances can reference.
(define (make-constructor struct-name fields options-map)
  (let ((options-name (concat-symbols '___ struct-name '-options___)))
    (list
      `(define ,options-name (hash ,@(hash->list options-map)))
      `(define ,struct-name
        (let ((options ,options-name))
          (lambda ,fields (mutable-vector
                    ___magic_struct_symbol___
                    (quote ,struct-name)
                    options
                    ,@fields)))))))



;; Defines the getters for each index. Maps at compile time the getter to the index in the vector
;; that contains the value. Take this for example:
;;
;;     (make-struct Applesauce (a b c))
;;
;; Once constructed like so: `(Applesauce 10 20 30),
;; under the hood, this is represented by a vector like so:
;; [___magic_struct_symbol___ 'Applesauce 10 20 30]
;;
;; With this, the function (Applesauce-a instance) maps to a simple index lookup in
;; the underlying vector, i.e. (mut-vector-ref instance 2)
(define (make-getters struct-name fields)
  (map (lambda (field)
         (let ((function-name (concat-symbols struct-name '- (car field)))
               (pred-name (concat-symbols struct-name '?)))
           `(define ,function-name
              (bind/c (make-function/c
                       (make/c ,pred-name (quote ,pred-name))
                       (make/c any/c 'any/c))
                      (lambda (this) (mut-vector-ref this ,(car (cdr field))))
                      (quote ,function-name)))))
       (enumerate 3 '() fields)))

;; Pretty much the same as the above, just now accepts a value to update
;; in the underlying struct.
(define (make-setters struct-name fields)
  (map (lambda (field)
         (let ((function-name (concat-symbols 'set- struct-name '- (car field) '!))
               (pred-name (concat-symbols struct-name '?)))
           `(define ,function-name
              (bind/c (make-function/c
                       (make/c ,pred-name (quote ,pred-name))
                       (make/c any/c 'any/c)
                       (make/c any/c 'any/c))
                      (lambda (this value) (vector-set! this ,(car (cdr field)) value))
                      (quote ,function-name)))))
       (enumerate 4 '() fields)))

;; Valid options on make-struct at the moment are:
;; :transparent, default #false
;; :printer, default doesn't exist
;; These will just be stored in a hash map at the back of the struct.
(define (make-struct struct-name fields . options)
  (when (not (list? fields))
    (error! "make-struct expects a list of field names, found " fields)
    void)

  (when (not (symbol? struct-name))
    (error! "make-struct expects an identifier as the first argument, found " struct-name))

  (when (odd? (length options))
    (error! "make-struct options are malformed - each option requires a value"))

  (let ((options-map (apply hash options)))
    `(begin
       ;; Constructor
       ,@(make-constructor struct-name fields options-map)
       ;; Predicate
       ,(make-predicate struct-name fields)
       ;; Getters here
       ,@(make-getters struct-name fields)
       ;; Setters
       ,@(make-setters struct-name fields))))

;; TODO: This is going to fail simply because when re-reading in the body of expanded functions
;; The parser is unable to parse already made un-parseable items. In this case, we should not
;; Re-parse the items, but rather convert the s-expression BACK into a typed ast instead.
; (define (%lambda% args body)
;   (let (
;         (non-default-bindings (filter (lambda (x) (not (pair? x))) args))
;         (bindings
;          (transduce
;           ;; Now we have attached the index of the list
;           ;; To the iteration
;           (enumerate 0 '() args)
;           ;; extract out the arguments that have a default associated
;           ;; So from the argument list like so:
;           ;; (a b [c <expr>] [d <expr>])
;           ;; We will get out ([c <expr>] [d <expr>])
;           (filtering (lambda (x) (pair? (car x))))
;           ;; Map to the let form of (binding expr)
;           (mapping (lambda (x)
;                      ;; ( (x, expr), index )
;                      ;; TODO: clean this up
;                      (let ((var-name (car (car x)))
;                            (expr (car (cdr (car x))))
;                            (index (car (cdr x))))
;                        `(,var-name (let ((,var-name (try-list-ref !!dummy-rest-arg!! ,index)))
;                                      (if ,var-name ,var-name ,expr))))))
;           (into-list))))

;     ;; TODO: Yes I understand this violates the macro writers bill of rights
;     ;; that being said I'm only doing this as a proof of concept anyway so it can be rewritten
;     ;; to be simpler and put the weight on the compiler later
;     (if (equal? (length args) (length non-default-bindings))
;         `(lambda ,args ,body)
;         ; (displayln "hello world")
;         `(lambda (,@non-default-bindings . !!dummy-rest-arg!!)
;            (let (,@bindings) ,body))
;         ;     ,(if bindings
;         ;         `(let (,@bindings) ,body)
;         ;         body))



;         )))