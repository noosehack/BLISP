;; Test axis directly with :row keyword
(defparameter df (stdin))
(print "Original sum (axis=:col):")
(print (sum df))

(defparameter df-row (o :row df))
(print "\nAfter (o :row df), sum should be M×1:")
(print (sum df-row))
