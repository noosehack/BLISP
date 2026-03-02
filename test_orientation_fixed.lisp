(defparameter df (stdin))

(print "=== ORIENTATION FIX VERIFICATION ===\n")

(print "1. Original table (axis=:col by default):")
(print df)

(print "\n2. Sum with axis=:col (down rows → 1×N):")
(defparameter sum-col (sum df))
(print sum-col)

(print "\n3. After (o 'Z df) - should set axis=:row:")
(defparameter df-z (o 'Z df))
(print df-z)

(print "\n4. Sum with axis=:row (across columns → M×1):")
(defparameter sum-row (sum df-z))
(print sum-row)

(print "\n=== EXPECTED RESULTS ===")
(print "- sum-col should have shape 1×3 (one row, three columns)")
(print "- sum-row should have shape 3×1 (three rows, one column)")
(print "\nIf both show 1×3, the fix didn't work.")
(print "If sum-row shows 3×1, the fix is working! ✅")
