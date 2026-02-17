; Test date display formatting
(defparameter data (file "test_with_dates.csv"))
(print "Date column:")
(print (col data "date"))
(print "Price column:")
(print (col data "price"))
