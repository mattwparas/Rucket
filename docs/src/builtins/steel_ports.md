# steel/ports
### **input-port?**
Checks if a given value is an input port

(input-port? any/c) -> bool?

#### Examples

```scheme
> (input-port? (stdin)) ;; => #true
> (input-port? "foo") ;; => #false
```
### **open-input-file**
Takes a filename `path` referring to an existing file and returns an input port. Raises an error
if the file does not exist

(open-input-file string?) -> input-port?

#### Examples
```scheme
> (open-input-file "foo-bar.txt") ;; => #<port>
> (open-input-file "file-does-not-exist.txt")
error[E08]: Io
┌─ :1:2
│
1 │ (open-input-file "foo-bar.txt")
│  ^^^^^^^^^^^^^^^ No such file or directory (os error 2)
```
### **open-output-file**
Takes a filename `path` referring to a file to be created and returns an output port.

(open-output-file string?) -> output-port?

#### Examples
```scheme
> (open-output-file "foo-bar.txt") ;; => #<port>
```
### **output-port?**
Checks if a given value is an output port

(output-port? any/c) -> bool?

#### Examples

```scheme
> (define output (open-output-file "foo.txt"))
> (output-port? output) ;; => #true
```
### **read-port-to-string**
Takes a port and reads the entire content into a string

(read-port-to-string port) -> string?

* port : input-port?
### **stdin**
Gets the port handle to stdin

(stdin) -> input-port?

#### Examples

```scheme
> (stdin) ;; => #<port>
```
### **flush-output-port**
### **get-output-string**
### **open-output-string**
### **raw-write**
### **raw-write-char**
### **raw-write-string**
### **read-line-from-port**
### **stdout**
### **write-line!**
