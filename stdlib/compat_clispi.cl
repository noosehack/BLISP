;; ---------------------------------------------
;; CLISPI → BLISP compatibility layer
;; Purpose: run CLISPI scripts on BLISP today.
;; Strategy:
;;   - Canonical IR names (dlog, cs1, shift, locf) that lack legacy builtins
;;     get macros mapping to their legacy *-cols equivalents for fallback
;;   - These macros are TRANSPARENT to IR: try_ir_eval sees raw AST,
;;     macros only expand during legacy rt.eval() fallback
;;   - Legacy spellings (*-cols) already exist as builtins — no macros needed
;;   - Genuinely useful sugar (avg, x-, ecs1, ur, etc.) is kept
;; ---------------------------------------------

;; =============================================================================
;; SECTION 0: THREADING MACRO (CRITICAL!)
;; =============================================================================

;; Threading macro is a BUILTIN special form in BLISP (not a macro)
;; (-> x (f a) (g b)) threads x through function calls
;; Implemented in src/eval.rs as eval_thread()

;; =============================================================================
;; SECTION 1: CANONICAL → LEGACY FALLBACK
;; =============================================================================

;; These canonical names exist in IR but NOT as legacy builtins.
;; When default HYBRID mode falls back to legacy (e.g. save wraps the expr),
;; these macros route to legacy *-cols builtins.
;;
;; IR-transparent: try_ir_eval receives raw AST, plans through IR.
;; Only if IR planning fails does legacy run, expanding these macros.
;;
;; When BLISP_SEGMENT=1 (segmented hybrid), these macros are bypassed
;; because hybrid_eval peels save and routes subtrees through IR directly.
;; In that mode, these macros are harmless but unused.

(defmacro dlog (x) `(dlog-cols ,x))
(defmacro cs1 (x) `(cs1-cols ,x))
(defmacro shift (x lag) `(shift-cols ,x ,lag))
(defmacro locf (x) `(locf-cols ,x))

;; > threshold: canonical (> x threshold) → legacy (>-cols x threshold)
(defmacro > (x threshold) `(>-cols ,x ,threshold))

;; ur: canonical data-first (ur x w step) → legacy prefix (ur-cols w step x)
(defmacro ur (x w step) `(ur-cols ,w ,step ,x))

;; =============================================================================
;; SECTION 2: NAME DIFFERENCES (genuinely useful sugar)
;; =============================================================================

;; --- Aggregates ---
(defmacro avg (x) `(mean ,x))           ; CLISPI avg → BLISP mean
(defmacro std_dev (x) `(std ,x))        ; CLISPI std_dev → BLISP std

;; --- Pairwise ---
;; x- → xminus also in canonicalize (src/normalize.rs), but macro kept
;; for legacy fallback path where canonicalize doesn't run.
(defmacro x- (x half) `(xminus ,x ,half))      ; x- → xminus

;; --- Rolling ---
(defmacro wavg (x w) `(wmean-cols ,x ,w))   ; CLISPI wavg → BLISP wmean-cols

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

;; --- Exponential Cumulative Sum (ecs1) ---
;; Reconstruct price index from log returns: exp(cumsum(dlog(prices)))
;; Inverse of dlog: if y = dlog(x), then x ≈ ecs1(y)
(defmacro ecs1 (x) `(exp (cs1 (dlog ,x))))

;; =============================================================================
;; SECTION 4: ORIENTATION (NOW A BUILTIN!)
;; =============================================================================

;; CLISPI patterns like (o WENS x) and (o ':row x) now work via builtin_o
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
