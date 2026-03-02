;; ES1I pipeline: locf → ft-zscore(250)
;; Matches: ES1I_locf_wzs_250_1.csv

(def es1i-data (read-csv "ES1I.csv"))

;; Apply locf (fill missing values)
(def es1i-locf (locf es1i-data))

;; Apply ft-zscore with window 250
;; ft-zscore = (x - rolling_mean_excl_current) / rolling_std_excl_current
(def es1i-mean (rolling-mean-partial-excl-current 250 es1i-locf))
(def es1i-std (rolling-std-partial-excl-current 250 es1i-locf))

;; Compute z-score: (x - mean) / std
(def es1i-centered (- es1i-locf es1i-mean))
(def es1i-zscore (/ es1i-centered es1i-std))

es1i-zscore
