(let ((data (locf (read-csv "/home/ubuntu/ES1I.csv"))))
  (let ((mean (rolling-mean-partial-excl-current 250 data))
        (std (rolling-std-partial-excl-current 250 data)))
    (/ (- data mean) std)))
