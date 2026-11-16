---
name: copy-guard
description: Fits on-screen copy to its time and slot limits by rewriting, never by shrinking type or speeding up.
---

# Copy guard

- **Budget per line:** `min(maxWords, floor(3 × (hold − 1)))` words, and at least 2.
- **Counting:** count tokens that contain a letter or digit ("10×" is 1 word; "—" is 0).
- **When over budget, cut in this order until it fits:**
  1. filler (very, really, simply, just, in order to);
  2. throat-clearing (Introducing, Meet, Now you can);
  3. stacked adjectives (keep the strongest);
  4. clauses turned into nouns ("lets you render faster" → "Faster renders");
  5. move the detail to another beat.
- **Keep** names and numbers exactly, and keep sibling lines parallel.

Output: `slot | hold | budget | before (n) | after (n) | OK/REWRITE/CUT`.
