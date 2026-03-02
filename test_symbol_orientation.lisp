(defparameter df (stdin))

(print "=== Testing Symbol-Based Orientation ===\n")

(print "1. Original (axis=:col):")
(defparameter sum-original (sum df))
(print sum-original)

(print "\n2. After (o 'H df) - should keep axis=:col:")
(defparameter df-h (o 'H df))
(defparameter sum-h (sum df-h))
(print sum-h)

(print "\n3. After (o 'Z df) - should set axis=:row:")
(defparameter df-z (o 'Z df))
(defparameter sum-z (sum df-z))
(print sum-z)

(print "\n=== VERIFICATION ===")
(print "sum-original shape: should be 1×3")
(print "sum-h shape: should be 1×3")
(print "sum-z shape: should be 3×1 ✅")
