from pkg import api, util


def both():
    return api() + str(util.helper(3))
