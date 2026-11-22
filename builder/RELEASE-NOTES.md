# Cinnamatica 0.3.60

Everything from 0.3.59, plus:

- **Placed and broken blocks reach the hologram sooner.** The mod gets 1,000,000 fuel a frame, and the check that notices a block placed or broken near you used to run late in the frame, after Fast Place and the section checks had spent part of it. It was skipped whenever less than 650,000 was left, and the ghost's update at the end of the frame needed 400,000 more, so on a busy frame a change simply waited. Now:
  - The check runs **first** in the frame, so it is never starved, and Fast Place decides on what the world holds now, not on what it held a few frames ago.
  - Its comparison is far cheaper. It used to rebuild a block id for every one of the 4,096 cells it watches, then compare them one by one; it now compares whole runs of 64 cells with one test each and looks closer only at the runs that differ. The same goes for the cells that both the old and the new box hold when you walk and the box moves. A test checks it against the old cell-by-cell answer over many random changes, unloaded cells, palette changes and box moves.
  - The lookup that decides what a cell's block means no longer uses the standard hash, which cost several times more fuel than the lookup itself.
  - The ghost's edits go to the game right after the check queues them, not at the end of the frame, and the end-of-frame send needs 300,000 left instead of 400,000. The outline redraw needs 500,000 instead of 700,000.

Verified: the mod's unit tests (96), the mod's real WASM build compiles (checked with a separate Rust 1.100 toolchain, which also now catches errors that only show in the WASM build). The full client was not built here. Not tried in the game: everything above.

---

# Cinnamatica 0.3.59

Everything from 0.3.57, plus:

- **A double chest gets one outline round both halves**, not one frame per half. Two halves of the same kind facing the same way, side by side, share one box while both are still missing. If one half is already placed, the other keeps its own frame.
- **The section check cannot be starved.** 0.3.57 made it wait for 1.5 times what a check cost. A mod that is always busy might never reach that, so sections would never be re-checked. After 24 frames in a row where it was held back, it uses the old fixed limit for one check.

Verified: the mod's unit tests (93, with new ones for the double chest frame and the fallback gate). The mod's WASM file and the full client were not built here. Not tried in the game: everything above.

