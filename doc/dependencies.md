# Dependency Justification

This document explains the heavy dependencies introduced for learning algorithm improvements.

## FSRS Crate (`fsrs = "5.2.0"`)

### Why FSRS?

FSRS (Free Spaced Repetition Scheduler) is a modern, research-backed spaced repetition algorithm that outperforms SM-2 (the algorithm used by Anki and our original implementation).

**Benefits:**
- **20-30% fewer reviews** for the same retention level (validated across 99% of users in benchmarks)
- **Better handling of irregular study patterns** (missed days, vacations)
- **Configurable retention target** (e.g., 90% retention)
- **Scientific foundation** - based on memory research and validated with millions of data points

### Heavy Dependencies Explained

The FSRS crate pulls in **burn** (a deep learning framework) because FSRS uses neural networks for:
1. **Parameter optimization** - Learning optimal scheduling weights from user review history
2. **Memory state calculation** - Computing stability/difficulty using trained models

**Key transitive dependencies:**
| Dependency | Purpose | Size Impact |
|------------|---------|-------------|
| `burn` | ML framework | ~15MB compiled |
| `burn-ndarray` | Tensor operations | ~5MB |
| `burn-core` | Core ML primitives | ~3MB |
| `ndarray` | N-dimensional arrays | ~2MB |

### Is the overhead worth it?

**Yes, for several reasons:**

1. **No runtime ML inference** for default parameters - FSRS comes with pre-trained default parameters that work well for most users. The heavy dependencies are only actively used when optimizing parameters (which happens in background after 1000+ reviews).

2. **Proven effectiveness** - FSRS is used by Anki (optional), Mnemosyne, and other SRS tools. The algorithm is open source and validated.

3. **One-time cost** - The dependencies only affect compile time and binary size, not runtime performance for normal operations.

### Alternatives Considered

| Alternative | Pros | Cons |
|-------------|------|------|
| Keep SM-2 only | No new deps | 20-30% more reviews needed |
| Implement FSRS manually | Smaller binary | Complex math, no optimization |
| FSRS-lite | Lighter deps | Less accurate, no param training |

We chose the full FSRS crate because:
- The parameter optimization capability is valuable for long-term users
- The extra compile time is acceptable for the learning benefits
- Binary size increase (~20MB) is acceptable for a learning app

### Mitigation

The app supports falling back to SM-2 via a setting (`use_fsrs = false`), so users can disable FSRS if binary size is a concern (though both are compiled in).

---

## Compile Time & Binary Size

**Before FSRS addition:**
- Compile time: ~30s
- Binary size: ~15MB

**After FSRS addition:**
- Compile time: ~3-5 minutes (first build)
- Binary size: ~35MB

**Notes:**
- Incremental builds remain fast (~3s) since FSRS doesn't change often
- Release builds with LTO can reduce binary size by ~30%
- Consider feature flags if binary size becomes critical

---

## References

- [FSRS Algorithm Paper](https://github.com/open-spaced-repetition/fsrs4anki/wiki/The-Algorithm)
- [SRS Benchmark Comparison](https://github.com/open-spaced-repetition/srs-benchmark)
- [FSRS Rust Implementation](https://github.com/open-spaced-repetition/fsrs-rs)
