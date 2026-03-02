; Test orientation refactor
(defparameter df (load "test_ori.csv"))

; Test 1: Print original (should be ori=H)
(print df)

; Test 2: Apply H orientation explicitly
(defparameter df-h (o 'H df))
(print df-h)

; Test 3: Apply Z orientation (transpose)
(defparameter df-z (o 'Z df))
(print df-z)

; Test 4: Sum with H orientation (down columns)
(defparameter sum-h (sum (o 'H df)))
(print sum-h)

; Test 5: Sum with Z orientation (across rows)
(defparameter sum-z (sum (o 'Z df)))
(print sum-z)

; Test 6: D4 composition (ro 'Z twice = identity)
(defparameter df-zz (ro 'Z (ro 'Z df)))
(print df-zz)
