; Test with-mask builtin
; 1. Load frame
; 2. Add weekend mask
; 3. Activate weekend mask
(with-mask (mask-weekend (file "../toto.csv")) 'weekend)
