;; BLISP Core Library
;; Basic macros and utilities

;; Increment
(defmacro inc (x)
  `(+ ,x 1))

;; Decrement
(defmacro dec (x)
  `(- ,x 1))

;; When (one-armed if)
(defmacro when (condition body)
  `(if ,condition ,body nil))

;; Unless (inverted one-armed if)
(defmacro unless (condition body)
  `(if ,condition nil ,body))

;; Conditional AND
(defmacro and (a b)
  `(if ,a ,b nil))

;; Conditional OR
(defmacro or (a b)
  `(if ,a ,a ,b))
