(defparameter df (stdin))

(print "Original table:")
(print df)

(print "\nWith R mode:")
(defparameter df-r (o 'R df))
(print df-r)

(print "\nSum of original (should be column sums):")
(print (sum df))

(print "\nSum of R mode (should be scalar?):")
(print (sum df-r))
