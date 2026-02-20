;; BLISP Finance Library (clispi-compatible)
;; Short names and macros for financial operations

;; ========== Aliases ==========

;; avg -> mean
(define avg (lambda (x) (mean x)))

;; std_dev -> std
(define std_dev (lambda (x) (std x)))

;; d -> diff (difference)
(define d (lambda (x y) (diff x y)))

;; ts -> shift (lag/lead alias)
(define ts (lambda (x y) (shift x y)))

;; ========== Macros ==========

;; Rolling z-score (wzs window step)
;; For step=1, use ft-wz0-cols directly
;; For step>1, use wz0-cols + keep-shape-cols + locf-cols
(defmacro wzs (data window step)
  `(if (== ,step 1)
       (ft-wz0-cols ,data ,window)
       (locf-cols (keep-shape-cols (wz0-cols ,data ,window) ,step))))

;; Rolling quality: inverse of volatility
(defmacro wq (data window step)
  `(/ 1 (wv-cols ,data ,window)))

;; Pairwise spreads
(defmacro xm (data half)
  `(xminus ,data ,half))

;; Rolling mean
(defmacro wm (data window step)
  `(ft-wmean-cols ,data ,window))

;; Information Ratio (Sharpe Ratio)
;; IR = mean / std * sqrt(260)
(defmacro ir (x)
  `(let* ((data ,x)
          (m (mean data))
          (s (std data)))
     (* (/ m s) 16.124515)))

;; Alias for backward compatibility
(defmacro ir2 (x)
  `(ir ,x))
