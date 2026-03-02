(defparameter df (stdin))

(print "Data shape:")
(print df)

(print "\n1. Default orientation (H - column-wise):")
(print "(sum df) - sum down rows, per column:")
(defparameter sum-h (sum df))
(print sum-h)

(print "\n2. Z orientation (row-wise):")
(print "(sum (o 'Z df)) - sum across columns, per row:")
(defparameter sum-z (sum (o 'Z df)))
(print sum-z)

(print "\n3. Are they different?")
(print "Same result?" )
;; Can't directly compare, but we can check shapes
(print "H sum shape:" )
(print sum-h)
(print "Z sum shape:")
(print sum-z)
