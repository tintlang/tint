# Tint Pong

Open at `/pong` in the sandbox.

The game loop, paddle AI, collision checks, keyboard controls, score, ball, and
paddles live in `pong.tn`. The board remains a declarative Tint grid, while the
moving entities are positioned overlays driven by continuous state. CSS
transitions smooth each frame, and every serve gets a different TN-level
direction and vertical angle.

No third-party copyrighted game assets are bundled here. The visual treatment
is original CSS/DOM artwork.
