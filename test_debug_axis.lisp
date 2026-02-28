(defparameter df (stdin))
(print "Created df")

(print "\nTrying (o 'Z df):")
(defparameter df-z (o 'Z df))
(print "Created df-z")

(print "\nChecking if they're the same:")
(print df)
(print df-z)
