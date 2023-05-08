(define print displayln)


(define (build-release)
    (~> (command "cargo" '("build" "--release"))
        (spawn-process)
        (Ok->value)
        (wait)))

(define (run-bench args)
    (~> (command "hyperfine" args)
        (spawn-process)
        (Ok->value)
        (wait)))

(define *interpreter-map*
    (hash "py" "python3.10"
          "scm" "../target/release/steel"
          "lua" "lua"))

(define (extension->interpreter ext)
    (hash-get *interpreter-map* ext))

(define (combine-interpreter-and-path interpreter path)
    (string-append (string-append interpreter " ") path))

(define (path->command-fragment path)
    (define interpreter (~> path 
                          (path->extension) 
                          (extension->interpreter)))
    (if (list? interpreter)
        (map (lambda (interp) (combine-interpreter-and-path interp path)) interpreter)
        (combine-interpreter-and-path interpreter path)))

(define (directory->bench-command dir)
    (flatten (map path->command-fragment (read-dir dir))))

(define (bench-group dir . options)
    (run-bench (append (filter (lambda (x) (not (ends-with? x ".lua"))) (directory->bench-command dir)) options)))

(define *benches*
    '(
        ("startup" "--warmup" "10" "--min-runs" "100" "--export-markdown" "warmup.md")
        ("map" "--warmup" "10")
        ("ack" "--warmup" "10")
        ("fib" "--warmup" "10" "--min-runs" "40" "--export-markdown" "fib.md")



    )

)

(define (main)
    (print "Building steel for release...")
    (build-release)
    (print "Running benches...")
    (transduce *benches* 
               (mapping (lambda (args) 
                                    (newline)    
                                    (apply bench-group args)))
               (into-list))

    ; (bench-group "bin-trees")
    (print "Done"))

(main)


; (define (main)
;     (displayln "Building steel for release...")
;     (build-release)
;     (displayln "Running benches...")
;     (run-bench "../target/release/steel startup/startup.scm" "python3.10 startup/startup.py" "--warmup" "10" "--min-runs" "100")
;     (run-bench "../target/release/steel fib/fib.scm" "python3.10 fib/fib.py" "--warmup" "10" "--min-runs" "40")
;     ; (run-bench '("../target/release/steel fib/fib.scm" "python3 fib/fib.py" "lua fib/fib.lua" "--warmup" "10" "--min-runs" "40"))
;     ; (run-bench '("../target/release/steel ack/ack.scm" "python3 ack/ack.py" "lua ack/ack.lua" "--warmup" "10" "--min-runs" "40"))
;     ; (run-bench '("../target/release/steel bin-trees/bin-trees.scm" "python3 bin-trees/bin_trees.py" "--warmup" "5"))
;     (displayln "Done"))

; (main)