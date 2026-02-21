# Session Start Checklist

**How to start a blisp session** - use git as source of truth.

## Quick Status Check (30 seconds)

```bash
cd /home/ubuntu/blisp

# 1. Recent work (last 10 commits)
git log --oneline -10

# 2. Current state
git status

# 3. Test health
cargo test 2>&1 | tail -5

# 4. Branch
git branch --show-current
```

This tells you:
- ✅ What was done recently
- ✅ What's uncommitted
- ✅ Tests passing or failing
- ✅ Which branch you're on

---

## Key Reference Documents

**Design Artifacts** (read when needed):
- `contracts.md` - Frozen semantic contracts (I1-I5, rolling ops, etc.)
- `BLISP_BLADE_Blueprint.txt` - Architecture plan (Phases 1-5)

**Status Documents** (milestone checkpoints):
- `BLADE_IR_STATUS.md` - IR implementation status
- `MILESTONE_4_COMPLETE.md` - Performance optimization milestone
- `IR_EXECUTOR_INTEGRATION.md` - IR executor wired into binary

**Verification**:
- `BLUEPRINT_VERIFICATION_REPORT.md` - Compliance check

**Project Overview**:
- `README.md` - Getting started

---

## What NOT to Create

❌ CURRENT_STATUS.md - use git log
❌ TODO.md - use git or just do it
❌ SESSION_NOTES.md - use git commits
❌ PROGRESS_*.md - use git log
❌ STEP_*_COMPLETE.md - redundant with commits

**Rule**: If git already tracks it, don't document it separately.

---

## Typical Session Flow

1. **Check git status** (see above)
2. **Ask user**: "What are we working on today?"
3. **Do the work**: code, test, iterate
4. **Commit frequently**: clear messages, co-authored
5. **Update milestone docs** only when crossing major thresholds

---

## When to Update Docs

**contracts.md**: Only when contracts change (rare, frozen)
**BLADE_IR_STATUS.md**: When major IR milestones complete
**README.md**: When user-facing features change

**Most work**: Just commit to git with good messages.

---

*Session start routine established: 2026-02-21*
