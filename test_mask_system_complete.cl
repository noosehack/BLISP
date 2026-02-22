; Complete mask system test
; Matches CLISPI: locf → w5 → dlog → cs1 → wavg(250)
; In BLISP: locf → mask-weekend → with-mask → dlog → cs1 → rolling-mean-partial(250)

(rolling-mean-partial 250
  (ecs1
    (dlog
      (with-mask
        (mask-weekend
          (locf (file "../toto.csv")))
        'weekend))))
