# Thesis

Flying Saucers keeps its world on the screen. Robert Arnstein's 3,345 bytes
hold a position for every saucer and every shot, but whether two things meet
is never worked out from those positions. A shot that is about to move looks
at the character already in video RAM where it is going; if anything is
there, it has hit something, and only then does the program search its table
to learn which saucer it was. A saucer does the same and, if it finds anything
but a blank or a shot, it has crashed. And a shot has no direction of its own. Every time it
moves, it moves the way the cannon is pointing at that instant, so turning the
cannon bends every shot in the air. The thing the box promised ("the aiming
keys can also change the direction of the missile after the missile has left
the cannon") is not a feature that was added; it is what happens when a shot
is just a character on a screen that asks the cannon which way to go.

# Outline (this volume's signature chapter)

**The Screen Is the World** — how Flying Saucers moves, steers and collides.
Purpose: show, with live moments, that position, collision and steering all
come from reading video RAM back, and why that fits a 4K machine.

1. *On screen* — a shot fired straight up, bent left then right in flight.
   (moment: steer)
2. *Overview* (Player route) — the idea without code.
3. *One pass, three clocks* — UPDATE's three countdowns; the pass as the unit
   of time; what difficulty changes. (figures d1/d5/d9; moment: clocks)
4. *A saucer is a string* — DRAW_SAUCER: six characters with blanks at both
   ends erase the trail; the lights row alternates. (moment: saucer steps,
   frame by frame)
5. *Look before you move* — the saucer's look-ahead: blank or shot, go;
   anything else, explode, no points. (moment: crash)
6. *The shot that asks the cannon* — MOVE_SHOTS and STEP; fragments borrow
   AIM. (moment: steer, frame by frame)
7. *What did I hit?* — FIND_TARGET: a hit is a non-blank cell; the saucer is
   found by row and a six-cell span; the hit pauses the game. (moment: hit and
   fragments; moment: chain)
8. *Why the screen* — what the design saved and what it cost, measured.

Routes: Player — 1, 2, 8. Period programmer — all. Modern programmer — 2, 6, 7, 8.

Planned moments: steer, clocks, saucer-step, crash, hit, chain, super.
