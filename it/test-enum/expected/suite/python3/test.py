import enum

class Entry:
  def __init__(self, field):
    self.field = field

  def __repr__(self):
    return "<Entry field: {!r}>".format(self.field)

Entry = enum.Enum("Entry", [("A", ("foo")), ("B", ("bar"))], type=Entry)
