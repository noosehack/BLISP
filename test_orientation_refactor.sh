#!/bin/bash
# Test the orientation refactor

set -e

echo "=== Test 1: Basic orientation (o 'H) ==="
echo '(defparameter df (stdin))
(print df)
(print (o (quote H) df))' | /home/ubuntu/blisp/target/release/blisp <<EOF
a,b,c
1.0,2.0,3.0
4.0,5.0,6.0
EOF

echo ""
echo "=== Test 2: Transpose (o 'Z) ==="
echo '(defparameter df (stdin))
(defparameter df-z (o (quote Z) df))
(print df-z)' | /home/ubuntu/blisp/target/release/blisp <<EOF
a,b,c
1.0,2.0,3.0
4.0,5.0,6.0
EOF

echo ""
echo "=== Test 3: Sum with H orientation (sum down columns) ==="
echo '(defparameter df (stdin))
(defparameter result (sum (o (quote H) df)))
(print result)' | /home/ubuntu/blisp/target/release/blisp <<EOF
a,b,c
1.0,2.0,3.0
4.0,5.0,6.0
EOF

echo ""
echo "=== Test 4: Sum with Z orientation (sum across rows) ==="
echo '(defparameter df (stdin))
(defparameter result (sum (o (quote Z) df)))
(print result)' | /home/ubuntu/blisp/target/release/blisp <<EOF
a,b,c
1.0,2.0,3.0
4.0,5.0,6.0
EOF

echo ""
echo "=== Test 5: D4 composition (ro) - Z o Z = H ==="
echo '(defparameter df (stdin))
(defparameter df-z (o (quote Z) df))
(defparameter df-h (ro (quote Z) df-z))
(print df-h)' | /home/ubuntu/blisp/target/release/blisp <<EOF
a,b,c
1.0,2.0,3.0
4.0,5.0,6.0
EOF

echo ""
echo "=== Test 6: All 8 D4 orientations ==="
for ori in H N _N _H Z S _Z _S; do
    echo "--- Orientation: $ori ---"
    echo "(defparameter df (stdin))
(print (o (quote $ori) df))" | /home/ubuntu/blisp/target/release/blisp <<EOF
a,b
1.0,2.0
3.0,4.0
EOF
done

echo ""
echo "=== All tests passed! ==="
