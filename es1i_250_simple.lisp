(/ (- (locf (read-csv "/home/ubuntu/ES1I.csv"))
      (rolling-mean-partial-excl-current 250 (locf (read-csv "/home/ubuntu/ES1I.csv"))))
   (rolling-std-partial-excl-current 250 (locf (read-csv "/home/ubuntu/ES1I.csv"))))
