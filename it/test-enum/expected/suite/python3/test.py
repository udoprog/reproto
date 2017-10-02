import enum

class Entry:
  def __init__(self, value):
    self.value = value

  def encode(self):
    return self.value

  @classmethod
  def decode(cls, data):
    for value in cls.__members__.values():
      if value.value == data:
        return value

    raise Exception("data does not match enum")

  def __repr__(self):
    return "<Entry value: {!r}>".format(self.value)

class Entry2:
  def __init__(self, value):
    self.value = value

  def encode(self):
    return self.value

  @classmethod
  def decode(cls, data):
    for value in cls.__members__.values():
      if value.value == data:
        return value

    raise Exception("data does not match enum")

  def __repr__(self):
    return "<Entry2 value: {!r}>".format(self.value)

Entry = enum.Enum("Entry", [("A", "foo"), ("B", "bar")], type=Entry)

Entry2 = enum.Enum("Entry2", [("A", "A"), ("B", "B"), ("C", "C")], type=Entry2)
