(defparameter df (stdin))

(print "=== At.csv Orientation Test ===\n")
(print "Data shape:")
(print df)

(print "\n1. H orientation (column sums):")
(defparameter sum-h (sum df))
(print sum-h)

(print "\n2. Z orientation (row sums):")
(defparameter sum-z (sum (o 'Z df)))
(print sum-z)

(print "\n3. R orientation (grand total):")
(defparameter sum-r (sum (o 'R df)))
(print sum-r)

(print "\n=== Test Complete ===")
