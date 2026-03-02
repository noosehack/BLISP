; Test basic mask operations
; Load → locf → mask-weekend → with-mask → dlog → ecs1

(ecs1
  (dlog
    (with-mask
      (mask-weekend
        (locf (file "../toto.csv")))
      'weekend)))
