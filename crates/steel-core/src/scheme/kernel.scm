;; The kernel for Steel.
;; This contains core forms that are expanded during the last phase of macro expansion
;; Macros that are exported from the kernel and applied on code externally are defined via
;; the form `(#%define-syntax <macro> <body>)`.
;;
;; This makes this function publicly available for the kernel to expand
;; forms with.

; (define *transformer-functions* (hashset))

;; Compatibility layers for making defmacro not as painful
(define displayln stdout-simple-displayln)

(define-syntax #%syntax-transformer-module
  (syntax-rules (provide)

    [(#%syntax-transformer-module name (provide ids ...) funcs ...)
     (define (datum->syntax name)
       (let ()
         (begin
           funcs ...)
         (#%syntax-transformer-module provide ids ...)))]

    ;; Normal case
    [(#%syntax-transformer-module provide name) (%proto-hash% 'name name)]

    ;; Normal case
    [(#%syntax-transformer-module provide name rest ...)
     (%proto-hash-insert% (#%syntax-transformer-module provide rest ...) 'name name)]))

;; Loading macros via defmacro - there will be a pass where we lower anything with defmacro down to the kernel,
;; which will then load and register macros accordingly.
(define-syntax defmacro
  (syntax-rules ()
    [(defmacro environment (name arg) expr)
     (begin
       (register-macro-transformer! (symbol->string 'name) environment)
       (define (name arg)
         expr))]

    [(defmacro environment (name arg) exprs ...)
     (begin
       (register-macro-transformer! (symbol->string 'name) environment)
       (define (name arg)
         exprs ...))]

    [(defmacro environment name expr)
     (begin
       (register-macro-transformer! (symbol->string 'name) environment)
       (define name expr))]))

(define-syntax #%define-syntax
  (syntax-rules ()

    [(#%define-syntax (name arg) expr)
     (begin
       (register-macro-transformer! (symbol->string 'name) "default")
       (define (name arg)
         expr))]

    [(#%define-syntax (name arg) exprs ...)
     (begin
       (register-macro-transformer! (symbol->string 'name) "default")
       (define (name arg)
         exprs ...))]

    [(#%define-syntax name expr)
     (begin
       (register-macro-transformer! (symbol->string 'name) "default")
       (define name expr))]))

;; Kernal-lambda -> Used in the meantime while `lambda` finds its way out of the reserved keywords.
(define-syntax klambda
  (syntax-rules ()
    [(klambda () expr exprs ...) (#%plain-lambda () expr exprs ...)]
    [(klambda (x xs ...) expr exprs ...) (#%plain-lambda (x xs ...) expr exprs ...)]
    [(klambda x expr exprs ...) (#%plain-lambda x expr exprs ...)]))

;; Enumerate the given list starting at index `start`
(define (enumerate start accum lst)
  (if (empty? lst)
      (reverse accum)
      (enumerate (+ start 1) (cons (list (car lst) start) accum) (cdr lst))))

(define (hash->list hm)
  (transduce (transduce hm (into-list))
             (mapping (lambda (pair)
                        ;; If we have a symbol as a key, that means we need to
                        ;; quote it before we put it back into the map
                        (if (symbol? (car pair))
                            ;; TODO: @Matt - this causes a parser error
                            ;; (cons `(quote ,(car x)) (cdr x))
                            (list (list 'quote (car pair)) (cadr pair))
                            pair)))
             (flattening)
             (into-list)))

(define (mutable-keyword? x)
  (equal? x '#:mutable))
(define (transparent-keyword? x)
  (equal? x '#:transparent))

(#%define-syntax (struct expr)
                 (define unwrapped (syntax-e expr))
                 (define struct-name (syntax->datum (second unwrapped)))
                 (define fields (syntax->datum (third unwrapped)))
                 (define options
                   (let ([raw (cdddr unwrapped)])
                     ; (displayln raw)
                     (if (empty? raw) raw (map syntax->datum raw))))
                 (define result (struct-impl struct-name fields options))
                 (syntax/loc result
                   (syntax-span expr)))

;; Macro for creating a new struct, in the form of:
;; `(struct <struct-name> (fields ...) options ...)`
;; The options can consist of the following:
;;
;; Single variable options (those which their presence indicates #true)
;; - #:mutable
;; - #:transparent
;;
;; Other options must be presented as key value pairs, and will get stored
;; in the struct instance. They will also be bound to the variable
;; ___<struct-name>-options___ in the same lexical environment where the
;; struct was defined. For example:
;;
;; (Applesauce (a b c) #:mutable #:transparent #:unrecognized-option 1234)
;;
;; Will result in the value `___Applesauce-options___` like so:
;; (hash #:mutable #true #:transparent #true #:unrecognized-option 1234)
;;
;; By default, structs are immutable, which means setter functions will not
;; be generated. Also by default, structs are not transparent, which means
;; printing them will result in an opaque struct that does not list the fields
(define (struct-impl struct-name fields options)

  ;; Add a field for storing the options, and for the index to the func
  (define field-count (length fields))
  ;; Mark whether this is actually a mutable and transparent struct, and
  ;; then drain the values from the
  (define mutable? (contains? mutable-keyword? options))
  (define transparent? (contains? transparent-keyword? options))

  (define options-without-single-keywords
    (transduce options
               (filtering (lambda (x) (not (mutable-keyword? x))))
               (filtering (lambda (x) (not (transparent-keyword? x))))
               (into-list)))

  (define default-printer-function
    (if transparent?
        `(lambda (obj printer-function)
           (display "(")
           (display (symbol->string ,(list 'quote struct-name)))
           ,@(map (lambda (field)
                    `(begin
                       (display " ")
                       (printer-function (,(concat-symbols struct-name '- field) obj))))
                  fields)

           (display ")"))

        #f))

  ;; Set up default values to go in the table
  (define extra-options
    (hash '#:mutable
          mutable?
          '#:transparent
          transparent?
          '#:fields
          (list 'quote fields)
          '#:name
          (list 'quote struct-name)))

  (when (not (list? fields))
    (error! "struct expects a list of field names, found " fields))

  (when (not (symbol? struct-name))
    (error! "struct expects an identifier as the first argument, found " struct-name))

  (when (odd? (length options-without-single-keywords))
    (error! "struct options are malformed - each option requires a value"))

  ;; Update the options-map to have the fields included
  (let* ([options-map (apply hash options-without-single-keywords)]
         [options-map (hash-union options-map extra-options)]
         [options-map (if (hash-try-get options-map '#:printer)
                          options-map
                          (hash-insert options-map '#:printer default-printer-function))]
         [maybe-procedure-field (hash-try-get options-map '#:prop:procedure)])

    (when (and maybe-procedure-field (> maybe-procedure-field (length fields)))
      (error! "struct #:prop:procedure cannot refer to an index that is out of bounds"))

    (define struct-options-name (concat-symbols '___ struct-name '-options___))
    (define struct-prop-name (concat-symbols 'struct: struct-name))
    (define struct-predicate (concat-symbols struct-name '?))

    `(begin
       ; (#%black-box "STRUCT" (quote ,struct-name))
       (define ,struct-options-name (hash ,@(hash->list options-map)))
       (define ,struct-name 'unintialized)
       (define ,struct-prop-name 'uninitialized)
       (define ,struct-predicate 'uninitialized)
       ,@(map (lambda (field)
                `(define ,(concat-symbols struct-name '- field)
                   'uninitialized))
              fields)
       ;; If we're mutable, set up the identifiers to later be `set!`
       ;; below in the same scope
       ,@(if mutable?
             (map (lambda (field)
                    `(define ,(concat-symbols 'set- struct-name '- field '!)
                       'unintialized))
                  fields)
             (list))

       (%plain-let
        ([prototypes (make-struct-type (quote ,struct-name) ,field-count)])
        (%plain-let
         ([struct-type-descriptor (list-ref prototypes 0)] [constructor-proto (list-ref prototypes 1)]
                                                           [predicate-proto (list-ref prototypes 2)]
                                                           ;; TODO: Deprecate this
                                                           [getter-proto (list-ref prototypes 3)]
                                                           [getter-proto-list
                                                            (list-ref prototypes 4)])
         (set! ,struct-prop-name struct-type-descriptor)
         (#%vtable-update-entry! struct-type-descriptor ,maybe-procedure-field ,struct-options-name)
         ,(if mutable?
              `(set! ,struct-name
                     (lambda ,fields (constructor-proto ,@(map (lambda (x) `(#%box ,x)) fields))))

              `(set! ,struct-name constructor-proto))
         ,(new-make-predicate struct-predicate struct-name fields)
         ,@
         (if mutable? (mutable-make-getters struct-name fields) (new-make-getters struct-name fields))
         ;; If this is a mutable struct, generate the setters
         ,@(if mutable? (mutable-make-setters struct-name fields) (list))
         void)))))

(define (new-make-predicate struct-predicate-name struct-name fields)
  `(set! ,struct-predicate-name predicate-proto))

(define (mutable-make-getters struct-name fields)
  (map (lambda (field)
         `(set! ,(concat-symbols struct-name '- (car field))
                (lambda (this) (#%unbox (getter-proto this ,(list-ref field 1))))))
       (enumerate 0 '() fields)))

(define (mutable-make-setters struct-name fields)
  (map (lambda (field)
         `(set! ,(concat-symbols 'set- struct-name '- (car field) '!)
                (lambda (this value) (#%set-box! (getter-proto this ,(list-ref field 1)) value))))
       (enumerate 0 '() fields)))

(define (new-make-getters struct-name fields)
  (map (lambda (field)
         `(set! ,(concat-symbols struct-name '- (car field))
                (list-ref getter-proto-list ,(list-ref field 1))
                ; (lambda (this) (getter-proto this ,(list-ref field 1)))
                ))
       (enumerate 0 '() fields)))

(define (new-make-setters struct-name fields)
  (map (lambda (field)
         `(set! ,(concat-symbols 'set- struct-name '- (car field) '!)
                (lambda (this value) (setter-proto this ,(list-ref field 1) value))))
       (enumerate 0 '() fields)))

(define (%make-memoize f)
  (lambda n
    (let ([previous-value (%memo-table-ref %memo-table f n)])
      (if (and (Ok? previous-value) (Ok->value previous-value))
          (begin
            ; (displayln "READ VALUE: " previous-value " with args " n)
            (Ok->value previous-value))
          (let ([new-value (apply f n)])
            ; (displayln "SETTING VALUE " new-value " with args " n)
            (%memo-table-set! %memo-table f n new-value)
            new-value)))))

(define *gensym-counter* 0)
(define (gensym)
  (set! *gensym-counter* (+ 1 *gensym-counter*))
  (string->symbol (string-append "##gensym" (to-string *gensym-counter*))))

;; TODO: @Matt -> for whatever reason, using ~> plus (lambda (x) ...) generates a stack overflow... look into that
(define (make-unreadable symbol)
  (~>> symbol (symbol->string) (string-append "##") (string->symbol)))

(#%define-syntax (define-values expr)
                 ; (displayln expr)
                 (define underlying (syntax-e expr))
                 (define bindings (syntax->datum (second underlying)))
                 (define expression (third underlying))
                 (define unreadable-list-name
                   (make-unreadable '#%proto-define-values-binding-gensym__))
                 `(begin
                    (define ,unreadable-list-name ,expression)
                    ,@(map (lambda (binding-index-pair)
                             `(define ,(car binding-index-pair)
                                (list-ref ,unreadable-list-name ,(list-ref binding-index-pair 1))))
                           (enumerate 0 '() bindings))))

(#%define-syntax (#%better-lambda expr) (quasisyntax (list 10 20 30)))

;; TODO: make this not so suspect, but it does work!
(#%define-syntax (#%current-kernel-transformers expr)
                 (cons 'list (map (lambda (x) (list 'quote x)) (current-macro-transformers!))))

(#%define-syntax (#%fake-lambda expr)
                 (define underlying (syntax-e expr))
                 (define rest (cdr underlying))
                 (cons '#%plain-lambda rest))
