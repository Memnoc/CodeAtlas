import pkg.util as pu


def dotted_alias():
    # The alias replaces the dotted path outright: `pkg.util.helper()` is not
    # legal here, and `pu` is the only receiver a call site can write.
    return pu.helper(6)
