"""Top-level shim so `python agents/main.py` still works."""
from ail_agent.__main__ import main

if __name__ == "__main__":
    import sys
    sys.exit(main())
