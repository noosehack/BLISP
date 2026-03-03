# BLISP Operation Inventory
# Generated: 2026-03-03 12:16:41 UTC

## 1. Legacy Builtins (src/builtins.rs)

     1	1023:keep_shape
     2	1055:keep_shape_cols
     3	1103:wkd
     4	1219:mask_weekend
     5	1335:with_mask
     6	1618:xminus
     7	1690:cs1
     8	1725:cs1_cols
     9	1834:ecs1
    10	1869:ecs1_cols
    11	2007:mapr
    12	2104:asofr
    13	2135:ur
    14	2225:ur_cols
    15	2339:wz0
    16	2403:wz0_cols
    17	2477:wzs
    18	2509:dlog
    19	2546:shift
    20	2573:diff
    21	257:add
    22	2599:file
    23	2628:file_head
    24	2658:stdin
    25	2670:save
    26	2715:col
    27	2753:w
    28	2783:setcol
    29	2860:withcol
    30	2934:make_col
    31	2954:print
    32	2966:type_of
    33	2975:len
    34	2997:sum
    35	3021:sum0
    36	3036:mean
    37	305:sub
    38	3127:mean0
    39	3142:std
    40	3265:std0
    41	3299:wstd
    42	3315:wstd0
    43	3331:wstd_cols
    44	3353:wstd0_cols
    45	336:mul
    46	3375:wv
    47	3402:wv_cols
    48	3435:zscore
    49	3493:chop
    50	3894:cols
    51	3915:select
    52	3946:select_num
    53	3966:map_cols
    54	4023:apply_cols
    55	4076:dlog_cols
    56	4105:shift_cols
    57	4131:diff_cols
    58	4181:o
    59	4257:ro
    60	533:div
    61	594:log
    62	611:exp
    63	628:abs
    64	653:gt
    65	694:lt
    66	732:gte
    67	770:lte
    68	808:eq
    69	846:neq
    70	887:gt_cols
    71	939:locf
    72	973:locf_cols

## 2. IR Planner Tokens (src/planner.rs)

     1	*
     2	-
     3	/
     4	Warning: 'dlog-col' is deprecated, use 'dlog' instead
     5	Warning: 'shift-col' is deprecated, use 'shift' instead
     6	Warning: 'ur-col' is deprecated, use 'ur' instead
     7	abs
     8	and
     9	asofr
    10	dlog
    11	dlog-col
    12	dlog-ofs
    13	exp
    14	file
    15	ft-mean
    16	ft-mean expects 2 arguments: (ft-mean w x)
    17	ft-std
    18	ft-std expects 2 arguments: (ft-std w x)
    19	ft-zscore
    20	inv
    21	keep
    22	keep expects 2 arguments: (keep k x)
    23	lag-obs expects 2 arguments: (lag-obs k x)
    24	let
    25	locf
    26	log
    27	mapr
    28	mask expr 'and' expects at least 1 argument
    29	mask expr 'not' expects 1 argument
    30	mask expr 'or' expects at least 1 argument
    31	mask-weekend
    32	not
    33	or
    34	ret
    35	rolling-mean
    36	rolling-std
    37	shift
    38	shift expects 2 arguments: (shift k x)
    39	shift-col
    40	shift-obs
    41	sqrt
    42	stdin
    43	stdin expects no arguments
    44	ur
    45	ur expects 3 arguments: (ur w step x)
    46	ur-col
    47	with-mask
    48	wkd
    49	wzs
    50	xminus
    51	{} expects 1 argument

## 3. Canonical IDs (NumericFunc in src/ir.rs)

     1	MSK_WKE
     2	MSK_WKE_DEF {
     3	SHF_PFX_LIN_SUM
     4	SHF_PTW_LIN_SHF { k: usize }
     5	SHF_PTW_LIN_SPR {
     6	SHF_PTW_OBS_NLN_DLOG
     7	SHF_PTW_OFS_NLN_DLOG
     8	SHF_REC_NLN_LOCF
     9	SHF_WIN_LIN_AVG { w: usize }
    10	SHF_WIN_MIN2_LIN_AVG { w: usize }
    11	SHF_WIN_MIN2_LIN_AVG_EXCL { w: usize }
    12	SHF_WIN_MIN2_NLN_SDV { w: usize }
    13	SHF_WIN_MIN2_NLN_SDV_EXCL { w: usize }
    14	SHF_WIN_NLN_SDV { w: usize }

## 4. Builtin Registry Tokens (get_builtin function)

