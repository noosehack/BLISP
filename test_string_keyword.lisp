(defparameter df (stdin))
(print "Original sum:")
(print (sum df))

(defparameter df-row (o ":row" df))
(print "\nAfter (o \":row\" df):")
(print (sum df-row))
