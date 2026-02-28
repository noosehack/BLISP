(defparameter df (stdin))

(print "=== PROOF OF BUG ===\n")

(print "1. Original (ori=H):")
(print df)

(print "\n2. After (o 'Z df) - layout set to Z but ori UNCHANGED:")
(defparameter df-z (o 'Z df))
(print df-z)

(print "\n3. Both produce identical sum (proof layout is ignored):")
(print "  sum(df):")
(print (sum df))
(print "  sum(df-z):")
(print (sum df-z))
