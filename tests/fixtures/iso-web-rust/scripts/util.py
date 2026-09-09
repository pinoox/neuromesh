"""Shared tooling helper.

This file is intentionally byte-identical to the copy in the sibling isolation
fixture. Two unrelated projects very often carry the same helper, config, or
license file at the same relative path. `ingest_file_keep` returns early when a
path's stored hash equals the incoming hash, so without a clean project swap
the previous project's nodes for this path survive into the next project's
graph. That is the leak `cross_project_isolation` guards against — do not edit
one copy without editing the other.
"""

import os


def repo_root():
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def read_env(name, default=None):
    return os.environ.get(name, default)
