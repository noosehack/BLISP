(defparameter df (stdin))
(defparameter df-x (o 'X df))

(print "X mode shape:")
(print df-x)

(print "\nTrying sum on X mode (should error?):")
(print (sum df-x))
