# Cinnamatica 0.3.59

Everything from 0.3.57, plus:

- **A double chest gets one outline round both halves**, not one frame per half. Two halves of the same kind facing the same way, side by side, share one box while both are still missing. If one half is already placed, the other keeps its own frame.
- **The section check cannot be starved.** 0.3.57 made it wait for 1.5 times what a check cost. A mod that is always busy might never reach that, so sections would never be re-checked. After 24 frames in a row where it was held back, it uses the old fixed limit for one check.

Verified: the mod's unit tests (93, with new ones for the double chest frame and the fallback gate). The mod's WASM file and the full client were not built here. Not tried in the game: everything above.

