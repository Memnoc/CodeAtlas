from . import parse


def emit(text):
    return parse.parse_it(text)
