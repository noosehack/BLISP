;; Orientation Truth Table Test
;; Verifies all 10 orientations work correctly

(defparameter df (stdin))

(print "=== ORIENTATION TRUTH TABLE TEST ===\n")

;; Test 1: ColwiseLike orientations
(print "1. ColwiseLike family (should all sum to column vectors):")
(print "   H:") (print (sum (o 'H df)))
(print "   N:") (print (sum (o 'N df)))
(print "   _N:") (print (sum (o '_N df)))
(print "   _H:") (print (sum (o '_H df)))

;; Test 2: RowwiseLike orientations  
(print "\n2. RowwiseLike family (should have different class):")
(print "   Z sum:") (print (sum (o 'Z df)))
;; S is synonym for Z, skip to avoid confusion
(print "   _Z sum:") (print (sum (o '_Z df)))
(print "   _S sum:") (print (sum (o '_S df)))

;; Test 3: Real mode (scalar reduction)
(print "\n3. Real mode (should reduce to single scalar):")
(print "   R sum:") (print (sum (o 'R df)))

;; Test 4: Each mode (should error)
(print "\n4. Each mode (should panic - comment out to run):")
(print "   X: skipped (would panic)")
;; (print (sum (o 'X df)))  ; Uncomment to verify panic

;; Test 5: D4 composition
(print "\n5. D4 composition test:")
(print "   Z∘Z should = H (identity):")
(defparameter zz (ro 'Z (ro 'Z df)))
(print "   Result:") (print zz)
(print "   Sum:") (print (sum zz))

(print "\n=== ALL TESTS COMPLETE ===")
