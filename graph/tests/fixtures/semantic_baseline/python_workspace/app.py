from toolkit.helpers import helper as helper_alias
import utility

class Greeter:
    def format(self, value):
        return helper(value)

def helper(value):
    return value

def caller(value):
    return helper(value)
