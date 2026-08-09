def shout(name):
    return _decorate(name).upper()


def _decorate(name):
    return f"hello {name}"
