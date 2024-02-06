;; Download a package from git - These should be stored in a reasonable location, probably under the
;; $STEEL_HOME directory.

(require-builtin steel/process)
(require "steel/result")
(require "parser.scm")

(provide git-clone
         in-directory
         run-dylib-installation
         download-and-install-library
         download-cog-to-sources-and-parse-module)

;; Sources!
(define (append-with-separator path dir-name)
  (if (ends-with? path "/") (string-append path dir-name) (string-append path "/" dir-name)))

(define (path-from-steel-home dir)
  (~> "STEEL_HOME" (env-var) (append-with-separator dir)))

(define *COG_DIR* (path-from-steel-home "cogs"))
(define *COG-SOURCES* (path-from-steel-home "cog-sources"))
(define *NATIVE_SOURCES_DIR* (path-from-steel-home "sources"))

;;@doc
;; Most likely should use gix here instead of shelling out to git?
;; Use the sha to pin to a specific commit, if interested
(define (git-clone package-name https-address installation-dir #:sha (*sha* void))

  (define resulting-path (string-append installation-dir "/" package-name))

  (displayln "Fetching package from git: " package-name)

  ;; Delete the target directory if it already exists
  (when (path-exists? resulting-path)
    (display "Clearing the target directory since it already exists: ")
    (displayln resulting-path)
    (delete-directory! resulting-path))

  ;; Git clone command, run against specific directory. For now we're going to
  ;; naively install them all into the same spot.
  (~> (command "git" (list "clone" https-address resulting-path)) (spawn-process) (Ok->value) (wait))
  ;; If we have a SHA, check out that commit
  (when (not (void? *sha*))

    (display "...Checking out sha: ")
    (displayln *sha*)

    (~> (command "git" (list "checkout" *sha*))
        (in-directory resulting-path)
        (spawn-process)
        (Ok->value)
        (wait)))

  resulting-path)

;; Run the process in the given directory
(define (in-directory command directory)
  (set-current-dir! command directory)
  command)

;; Run the cargo-steel-lib installer in the target directory
(define (run-dylib-installation target-directory #:subdir [subdir ""])
  (wait (run-dylib-installation-in-background target-directory #:subdir subdir)))

(define (run-dylib-installation-in-background target-directory #:subdir [subdir ""])
  (define target (append-with-separator target-directory subdir))
  (displayln "Running dylib build in: " target)
  (~> (command "cargo-steel-lib" '()) (in-directory target) spawn-process Ok->value))

;;@doc
;; Download cog source to sources directory, and then install from there.
;; Returns the resulting cog module hash. Will fail if the subdirectory
;; given does not contain a cog.scm file.
(define (download-cog-to-sources-and-parse-module library-name
                                                  git-url
                                                  #:subdir [subdir ""]
                                                  #:sha [*sha* void])

  (~> (git-clone library-name git-url *COG-SOURCES* #:sha *sha*)
      ;; If we're attempting to install the package from a subdirectory of
      ;; git urls, we should do that accordingly here.
      (append-with-separator subdir)
      parse-cog
      car))

;; TODO: steps
;; - git clone to temporary directory (or site-packages style thing, something)
;; Probably install native dylibs to their own native section
;; Then, run the installation script.

;; Install to the im-lists directory. What we probably have to do is install it to some
;; temporary location, parse the module name, and move it back out. Unless - we do something
;; like the org name, but I don't love that.
; (git-clone "im-lists" "https://github.com/mattwparas/im-lists.git" *NATIVE_SOURCES_DIR*)

;;@doc
;; Download and install the dylib library!
(define (download-and-install-library library-name git-url #:subdir [subdir ""] #:sha [*sha* void])
  (~> (git-clone library-name git-url *NATIVE_SOURCES_DIR*) (run-dylib-installation #:subdir subdir)))

;; Grabs the latest from the git url, stores in sources, and runs the installation in the target directory
; (download-and-install-library "steel-sys-info"
;                               "https://github.com/mattwparas/steel.git"
;                               #:subdir "crates/steel-sys-info")
