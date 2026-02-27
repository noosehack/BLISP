;; ---------------------------------------------
;; CLISPI → BLISP compatibility layer
;; Purpose: run CLISPI scripts on BLISP today.
;; Strategy:
;;   - CLISPI surface verbs become macros that expand to BLISP builtins
;;   - Keep semantics (colwise default) at the surface
;;   - Temporary: x- expands to xminus until reader supports hyphen symbols
;; ---------------------------------------------

;; =============================================================================
;; SECTION 0: THREADING MACRO (CRITICAL!)
;; =============================================================================

;; Threading macro is now a BUILTIN special form in BLISP (not a macro)
;; (-> x (f a) (g b)) threads x through function calls
;; Implemented in src/eval.rs as eval_thread()
;; This macro definition is NOT used - kept for reference only
;;
;; (defmacro -> (x &rest forms) ...)  ;; NOT USED - builtin instead

;; =============================================================================
;; SECTION 1: OPERATIONS WITH OPTIONAL ARGUMENTS
;; =============================================================================

;; dlog, shift, diff now have optional lag arguments (default=1) at the builtin level
;; No compat macros needed - use the builtins directly:
;;   (dlog x)     → uses default lag=1
;;   (shift x 2)  → uses explicit lag=2
;;   (diff x)     → uses default lag=1

;; =============================================================================
;; SECTION 2: NAME DIFFERENCES (need macro mapping)
;; =============================================================================

;; --- Aggregates ---
(defmacro avg (x) `(mean ,x))           ; CLISPI avg → BLISP mean
(defmacro std_dev (x) `(std ,x))        ; CLISPI std_dev → BLISP std

;; --- Cumulative operations ---
;; cs1 - now in IR as NumericFunc::CumSum (Phase 2.2)
;; ecs1 - implemented as macro: exp(cs1(dlog(x))) (Phase 3)

;; --- Comparison operations ---
;; > now points to >-cols at the builtin level - no macro needed

;; --- Rolling operations ---
;; wzs - Windowed z-score (now a builtin, not a macro)
;; Usage: (wzs window step x)
;; Note: wzs is registered as builtin_wzs in src/builtins.rs
;; (defmacro wzs (w l x) `(rolling-zscore ,w ,x))  ; OLD - builtin exists now
(defmacro wavg (x w) `(wmean-cols ,x ,w))   ; CLISPI wavg → BLISP wmean-cols

;; --- Pairwise (temporary until reader supports x-) ---
(defmacro x- (x half) `(xminus ,x ,half))      ; x- → xminus (reader limitation)

;; --- Filter operations ---
(defmacro keep (x step) `(keep-shape ,x ,step)) ; CLISPI keep → BLISP keep-shape

;; =============================================================================
;; SECTION 3: COMPOSITE MACROS (implement via existing primitives)
;; =============================================================================

;; --- Rolling quality (from CLISPI finance_short.cl) ---
;; wq = 1 / rolling volatility
(defmacro wq (x w) `(/ 1 (wv ,x ,w)))

;; --- Information Ratio ---
;; IR = mean / std * sqrt(260) for daily returns
(defmacro ir (x)
  `(let* ((data ,x)
          (m (mean data))
          (s (std data)))
     (* (/ m s) 16.124515)))   ; sqrt(260) ≈ 16.124515

;; Alias for backward compat
(defmacro ir2 (x) `(ir ,x))

;; --- Unit Ratio (ur) ---
;; Unit ratio = value / (100 * sqrt(252) * rolling_std)
;; Used for risk-adjusted returns in GLD_NUM pipeline
;; Note: step parameter ignored (for future keep-shape support)
(defmacro ur (w step x)
  `(/ ,x (* 100.0 (* 15.874507866 (rolling-std ,w ,x)))))  ; sqrt(252) ≈ 15.874507866

;; --- Exponential Cumulative Sum (ecs1) ---
;; Reconstruct price index from log returns: exp(cumsum(dlog(prices)))
;; Inverse of dlog: if y = dlog(x), then x ≈ ecs1(y)
(defmacro ecs1 (x) `(exp (cs1 (dlog ,x))))

;; =============================================================================
;; SECTION 4: ORIENTATION (NOW A BUILTIN!)
;; =============================================================================

;; CLISPI patterns like (o WENS x) and (o ':row x) now work via builtin_o
;; The builtin was implemented in Phase B and supports:
;;   - Axis keywords: (o ':col x), (o ':row x), (o ':reset x)
;;   - Layout symbols: (o 'NSWE x), (o 'WENS x), (o 'H x), (o 'Z x)
;; No macro needed - builtin takes precedence

;; =============================================================================
;; SECTION 5: MISSING OPS (to be implemented in BLISP or added here)
;; =============================================================================

;; These operations are used in CLISPI but not yet in BLISP.
;; Mark them as TODOs for future implementation:

;; MISSING - Elementwise:
;;   - inv (1/x)           → needs BLISP builtin
;;   - sign                → needs BLISP builtin
;;   - one                 → needs BLISP builtin
;;   - pow                 → needs BLISP builtin (might exist as ^)
;;   - asc, dsc            → needs BLISP builtin
;;   - g (growth rate)     → needs BLISP builtin
;;   - ema                 → needs BLISP builtin
;;   - locb                → needs BLISP builtin

;; MISSING - Aggregates:
;;   - med (median)        → needs BLISP builtin
;;   - vol                 → needs BLISP builtin
;;   - min, max            → needs BLISP builtin

;; MISSING - Rolling:
;;   - uc (upside capture) → needs BLISP builtin
;;   - whr                 → needs BLISP builtin
;;   - wmax, wmin, wmed    → needs BLISP builtin
;;   - ur                  → ✅ IMPLEMENTED as macro (Phase 3)

;; MISSING - Pairwise:
;;   - xdiv, xplus, xmult  → needs BLISP builtin

;; MISSING - Combinatory:
;;   - minusx, plusx, divx, multx → needs BLISP builtin

;; MISSING - Logical:
;;   - logical_not, logical_or → needs BLISP builtin

;; MISSING - Filter:
;;   - keepnew, stutter    → needs BLISP builtin

;; MISSING - Mapping:
;;   - map, mapc           → needs BLISP builtin

;; MISSING - Debug:
;;   - dump                → can be implemented as (save filename x) followed by x

;; Temporary dump implementation (save and return)
(defmacro dump (filename x)
  `(let* ((temp ,x))
     (save ,filename temp)
     temp))
