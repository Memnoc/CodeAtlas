import os

from ns import nowhere
from os import path


def nothing():
    # `helper` is exported by pkg/util.py, but `os` is not a module in this
    # map, so resolving by callee name alone would invent the edge.
    return os.helper(), nowhere, path
