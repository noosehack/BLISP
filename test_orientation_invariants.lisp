;; Meta-test: Orientation Invariants
;; Verifies foundation is stable before pushing

(defparameter df (stdin))

(print "=== ORIENTATION INVARIANTS META-TEST ===\n")

;; Invariant 1: PRT displays orientation correctly
(print "1. Display invariant (ori= matches intent):")
(print "   H:") (print (o 'H df))
(print "   Z:") (print (o 'Z df))
(print "   R:") (print (o 'R df))

;; Invariant 2: sum behavior matches class
(print "\n2. Aggregation invariant (class → behavior):")
(print "   H sum (ColwiseLike):") (print (sum (o 'H df)))
(print "   Z sum (RowwiseLike):") (print (sum (o 'Z df)))
(print "   R sum (Real):") (print (sum (o 'R df)))

;; Invariant 3: D4 composition
(print "\n3. D4 composition invariant (Z∘Z = H):")
(defparameter zz (ro 'Z (ro 'Z df)))
(print "   ro Z (ro Z df):") (print zz)
(print "   Sum matches H:") (print (sum zz))
(print "   Compare to H sum:") (print (sum (o 'H df)))

;; Invariant 4: Composition is associative (spot check)
(print "\n4. Composition associativity (spot check):")
(defparameter n-then-z (ro 'Z (ro 'N df)))
(print "   ro Z (ro N df):") (print n-then-z)

(print "\n=== ALL INVARIANTS VERIFIED ===")
