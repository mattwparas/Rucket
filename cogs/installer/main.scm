(require "steel/command-line/args.scm")
(require "package.scm")
(require "parser.scm")
(require "download.scm")

(define my-options
  (make-command-line-arg-parser #:positional (list '("command" "The subcommand to run"))
                                ; #:required '((("list" #f) "Setting up the values")))
                                ))

(define list-parser
  (make-command-line-arg-parser #:required '((("path" #f) "Path to discover packages"))))

(define (list-packages index)
  (define package-name-width
    (apply max
           (map (lambda (package) (string-length (symbol->string (hash-ref package 'package-name))))
                (hash-values->list index))))

  (define version-width (string-length "Version"))

  (displayln "Listing packages from: " *STEEL_HOME*)
  (displayln)

  (display "Package")
  (display (make-string (- package-name-width (string-length "Package")) #\SPACE))
  (display "  ")
  (displayln "Version")

  (display (make-string package-name-width #\-))
  (display "  ")
  (displayln (make-string version-width #\-))

  ;; Installed packages
  (for-each (lambda (package)
              (define package-name (hash-ref package 'package-name))
              (display package-name)
              (display (make-string (- package-name-width
                                       (string-length (symbol->string package-name)))
                                    #\SPACE))
              (display " ")
              (displayln (hash-ref package 'version)))
            (hash-values->list index)))

;; TODO: Move this to `installer/package.scm`
(define (refresh-package-index index)
  (define package-spec
    (download-cog-to-sources-and-parse-module "steel/packages"
                                              "https://github.com/mattwparas/steel-packages.git"))
  (check-install-package index package-spec))

;; TODO: Move this to `installer/package.scm`
(define (list-package-index)
  ;; What is going on here?
  (eval '(require "steel/packages/packages.scm"))
  (eval 'package-index))

(define (print-package-index)
  (define package-index (list-package-index))
  (transduce package-index
             (into-for-each (lambda (p)
                              (display (car p))
                              (display " ")
                              (displayln (cadr p))))))

(define (install-package-temp index args)
  (define cogs-to-install (if (empty? args) (list (current-directory)) args))
  (transduce cogs-to-install
             (flat-mapping parse-cog)
             (into-for-each (lambda (x) (check-install-package index x)))))

(define (install-package-if-not-installed installed-cogs cog-to-install)
  (define package-name (hash-get cog-to-install 'package-name))
  (if (hash-contains? installed-cogs package-name)
      (displayln "Package already installed. Skipping installation.")
      (begin
        (displayln "Package is not currently installed.")
        (install-package-and-log cog-to-install))))

;; TODO: Move this to `installer/package.scm`
(define (install-package-from-pkg-index index package args)
  (define pkg-index (list-package-index))
  (define remote-pkg-spec (hash-ref pkg-index (string->symbol package)))
  (define git-url (hash-ref remote-pkg-spec '#:url))
  (define subdir (or (hash-try-get remote-pkg-spec '#:path) ""))
  ;; Pass the path down as well - so that we can install things that way
  (define package-spec (download-cog-to-sources-and-parse-module package git-url #:subdir subdir))

  (define force (member "--force" args))

  (if force
      (install-package-and-log package-spec)
      (install-package-if-not-installed index package-spec)))

(define (uninstall-package-from-index index package)
  (define pkg (if (symbol? package) package (string->symbol package)))
  (unless (hash-contains? index pkg)
    (displayln "Package not found:" package)
    (return! void))

  (define package (hash-ref index pkg))
  ;; TODO: Contracts called in tail position get the call site location
  ;; wrong, since that is removed from the stack.
  (uninstall-package package))

;; Automatically re-installing isn't good. We'll fix that.
(define (install-dependencies index args)
  ;; Find all the dependencies, install those
  (match args
    [(list)
     ;; Only discover top level projects here
     (define top-level-files (read-dir (current-directory)))
     ;; Are there any cog files here?
     (define cog-files (filter (lambda (x) (equal? (file-name x) "cog.scm")) top-level-files))
     (define spec (hash-insert (parse-cog-file (car cog-files)) 'path (current-directory)))
     (walk-and-install spec)
     (displayln "Package built!")]

    [(list package)
     ;; Get the passed in argument
     (define path-to-package (car args))
     (define spec (car (parse-cog path-to-package)))
     (walk-and-install spec)
     (displayln "Package built!")]))

;; Generate a directory with a cog.scm, a hello world, etc
(define (generate-new-project args)
  (error "Implement generate new project"))

(define (install-dependencies-and-run-entrypoint index args)
  (define top-level-files (read-dir (current-directory)))
  (define cog-files (filter (lambda (x) (equal? (file-name x) "cog.scm")) top-level-files))
  (define spec (hash-insert (parse-cog-file (car cog-files)) 'path (current-directory)))
  (define entrypoint (~> (apply hash (hash-ref spec 'entrypoint)) (hash-ref '#:path)))
  ;; Run the entrypoint specified
  (~> (command "steel" (list entrypoint)) spawn-process Ok->value wait))

(define (render-help)
  (displayln
   "Forge - the Steel Packager Manager

Usage:
  forge <command> [options]

Commands:
  new            Scaffold a new package
  list           List the installed packages
  install        Install a local package
  build          Install the dependencies for the package
  run            Run the entrypoint as specified in the package

  pkg <command>  Subcommand for the package repository
  pkg refresh    Update the package repository from the remote
  pkg list       List the packages in the remote index
  pkg install    Install a package from the remote index"))

(define (get-command-line-args)
  (define args (command-line))
  ;; Running as a program, vs embedded elsewhere?
  (if (ends-with? (car args) "steel") (drop args 2) (drop args 1)))

(provide main)
(define (main)
  (define package-index (discover-cogs *STEEL_HOME*))
  (define command-line-args (get-command-line-args))

  (when (empty? command-line-args)
    (render-help)
    (return! void))

  (let ([command (car command-line-args)])
    ;; Dispatch on the command
    (cond
      ;; Generate a new project
      [(equal? "new" command) (generate-new-project (cdr command-line-args))]
      [(equal? "help" command) (render-help)]
      [(equal? "--help" command) (render-help)]
      ;; list the packages
      [(equal? "list" command) (list-packages package-index)]

      ;; Build the dependencies
      [(equal? "build" command) (install-dependencies package-index (cdr command-line-args))]

      ;; Run the entrypoint as specified in the cog.scm, if present.
      ;; Only install dependencies if they haven't been installed before.
      ;; For packages that already exist, we should take what is there.
      ;;
      ;; Versioning is hard. This will have to come up with some versioning scheme
      ;; that makes sense.
      [(equal? "run" command)
       (install-dependencies-and-run-entrypoint package-index (cdr command-line-args))]

      ;; install the given package
      [(equal? "install" command) (install-package-temp package-index (cdr command-line-args))]
      ;; Try to install the package index from the remote
      [(equal? '("pkg" "refresh") command-line-args) (refresh-package-index package-index)]
      ;; List the remote package index
      [(equal? '("pkg" "list") command-line-args) (print-package-index)]

      ;; Install package from remote
      [(equal? '("pkg" "install") (take command-line-args 2))
       ;; Force a re-install
       (install-package-from-pkg-index package-index
                                       (list-ref command-line-args 2)
                                       (drop command-line-args 3))]

      [(equal? '("pkg" "uninstall") (take command-line-args 2))
       (uninstall-package-from-index package-index (list-ref command-line-args 2))]

      ;; No matching command
      [else (displayln "No matching command: " command)]))

  ;; Wait for the jobs to finish
  (wait-for-jobs))
