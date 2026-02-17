; Test F64 column display
(print "Testing F64 display:")
(defparameter prices (make-col 100.0 102.0 101.5 103.0 104.5))
(print prices)

(print "Creating larger column for truncation test:")
(defparameter large (make-col 1.0 2.0 3.0 4.0 5.0 6.0 7.0 8.0 9.0 10.0 11.0 12.0 13.0 14.0 15.0))
(print large)
