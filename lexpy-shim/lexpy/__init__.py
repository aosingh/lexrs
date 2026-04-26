import warnings

warnings.warn(
    "lexpy is now a thin wrapper around lexrs (Rust-backed). "
    "To drop the shim layer, switch to: from lexrs import Trie, DAWG",
    DeprecationWarning,
    stacklevel=2,
)

from lexrs import DAWG, Trie  # noqa: E402, F401
