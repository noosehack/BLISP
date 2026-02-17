; Test aggregation functions
(print "Testing sum and mean:")

; Column without NA
(defparameter prices (make-col 100.0 102.0 101.5 103.0 104.5))
(print prices)
(print "sum:")
(print (sum prices))
(print "mean:")
(print (mean prices))

; Column with NA in middle
(print "\nTesting with NA values:")
(defparameter data (file "test_na_dates.csv"))
(defparameter prices-na (col data 'price))
(print prices-na)

(print "sum (propagates NA):")
(print (sum prices-na))

(print "sum0 (ignores NA):")
(print (sum0 prices-na))

(print "mean (propagates NA):")
(print (mean prices-na))

(print "mean0 (ignores NA):")
(print (mean0 prices-na))
