class Helper:
    def helper(self, value):
        return value


def call_on_a_value(util):
    # `util` is a parameter holding an object, and `pkg/util.py` sits right
    # beside this file. A resolver that treats any dotted receiver as a module
    # path wires this call into that module — an edge the source does not
    # contain, from a file that imports nothing at all.
    return util.helper(1)
